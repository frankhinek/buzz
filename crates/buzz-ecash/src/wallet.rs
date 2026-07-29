//! Async e-cash wallet built on the fedimint client stack.
//!
//! A [`Wallet`] owns a fedimint client joined to a single federation. The
//! wallet directory holds the rocksdb client database (`client.db/`) and,
//! for the file-based flow ([`Wallet::join`] / [`Wallet::open`]), the BIP-39
//! mnemonic that derives the client's root secret (`mnemonic.txt`, mode
//! 0600). Callers that keep the mnemonic elsewhere (e.g. the desktop app's
//! OS keychain) use the injection flow instead:
//! [`Wallet::generate_mnemonic`], [`Wallet::join_with_mnemonic`], and
//! [`Wallet::open_with_mnemonic`] — no `mnemonic.txt` is ever written.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use fedimint_bip39::{Bip39RootSecretStrategy, Mnemonic};
use fedimint_client::module_init::ClientModuleInitRegistry;
use fedimint_client::secret::RootSecretStrategy as _;
use fedimint_client::{Client, ClientBuilder, ClientHandleArc, RootSecret};
use fedimint_connectors::ConnectorRegistry;
use fedimint_core::db::Database;
use fedimint_core::invite_code::InviteCode;
use fedimint_core::module::AmountUnit;
use fedimint_core::Amount;
use fedimint_ln_client::{LightningClientInit, LightningClientModule};
use fedimint_mint_client::{
    MintClientInit, MintClientModule, OOBNotes, ReissueExternalNotesState,
    SelectNotesWithAtleastAmount,
};
use futures_util::StreamExt as _;
use serde::Serialize;

use crate::error::WalletError;

/// File under the wallet data dir holding the BIP-39 mnemonic (mode 0600).
const MNEMONIC_FILE: &str = "mnemonic.txt";

/// Directory under the wallet data dir holding the rocksdb client database.
const CLIENT_DB_DIR: &str = "client.db";

/// BIP-39 word count used for generated wallet mnemonics.
const MNEMONIC_WORD_COUNT: usize = 12;

/// Summary information about an open wallet and its federation.
#[derive(Debug, Clone, Serialize)]
pub struct WalletInfo {
    /// Hex-encoded federation id.
    pub federation_id: String,
    /// Federation display name from config meta, if set.
    pub name: Option<String>,
    /// Bitcoin network of the federation's lightning module, if available.
    pub network: Option<String>,
    /// Total spendable e-cash balance in millisatoshis.
    pub balance_msat: u64,
}

/// Outcome of an out-of-band e-cash spend.
#[derive(Debug, Clone, Serialize)]
pub struct SpendResult {
    /// Serialized out-of-band notes — hand these to the recipient.
    pub notes: String,
    /// Operation id tracking the spend in the client's operation log.
    pub operation_id: String,
    /// Total value of the selected notes in millisatoshis. May exceed the
    /// requested amount when exact change was not available (the recipient
    /// receives the full value).
    pub amount_msat: u64,
}

/// An e-cash wallet joined to a single fedimint federation.
///
/// Supports out-of-band e-cash only (mint module); lightning modules are
/// registered so their federation config is understood, but no lightning
/// send/receive API is exposed yet. On-chain deposits/withdrawals are out of
/// scope (the wallet module is deliberately not registered).
///
/// Dropping the last handle shuts the underlying client down; requires a
/// multi-threaded tokio runtime (rocksdb and client shutdown block in place).
pub struct Wallet {
    client: ClientHandleArc,
}

impl Wallet {
    /// Generate a fresh 12-word BIP-39 mnemonic suitable for
    /// [`Wallet::join_with_mnemonic`].
    ///
    /// The caller owns persistence (e.g. the desktop stores it in the OS
    /// keychain). Nothing is written to disk by this call.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::Mnemonic`] if entropy generation fails.
    pub fn generate_mnemonic() -> Result<String, WalletError> {
        Ok(Mnemonic::generate(MNEMONIC_WORD_COUNT)
            .map_err(|e| WalletError::Mnemonic(e.to_string()))?
            .to_string())
    }

    /// Create a new wallet under `data_dir` and join the federation described
    /// by `invite_code`.
    ///
    /// Generates a fresh BIP-39 mnemonic, persists it to
    /// `data_dir/mnemonic.txt` (mode 0600 on unix), downloads the federation
    /// config via the invite code, and joins with the mint (primary),
    /// lightning, and lightning-v2 client modules registered.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::AlreadyInitialized`] if a wallet already exists
    /// at `data_dir`, [`WalletError::InvalidInviteCode`] if the invite code
    /// fails to parse, and [`WalletError::Federation`] if config download or
    /// the join itself fails.
    pub async fn join(data_dir: impl AsRef<Path>, invite_code: &str) -> Result<Self, WalletError> {
        let data_dir = data_dir.as_ref();
        let invite = parse_invite(invite_code)?;

        std::fs::create_dir_all(data_dir).map_err(|source| WalletError::DataDir {
            path: data_dir.to_path_buf(),
            source,
        })?;
        let mnemonic_file = mnemonic_path(data_dir);
        if mnemonic_file.exists() || data_dir.join(CLIENT_DB_DIR).exists() {
            return Err(WalletError::AlreadyInitialized(data_dir.to_path_buf()));
        }

        let mnemonic = Mnemonic::generate(MNEMONIC_WORD_COUNT)
            .map_err(|e| WalletError::Mnemonic(e.to_string()))?;
        write_mnemonic_file(&mnemonic_file, &mnemonic)?;

        match Self::join_parsed(data_dir, &invite, &mnemonic).await {
            Ok(wallet) => Ok(wallet),
            Err(e) => {
                // The dir was empty before this call, so a failed join must
                // leave it empty again — otherwise the leftover mnemonic
                // makes every retry fail AlreadyInitialized. (join_parsed
                // already removed the half-initialized db.)
                let _ = std::fs::remove_file(&mnemonic_file);
                Err(e)
            }
        }
    }

    /// Create a new wallet under `data_dir` using a caller-provided BIP-39
    /// `mnemonic` (e.g. from [`Wallet::generate_mnemonic`]) and join the
    /// federation described by `invite_code`.
    ///
    /// Unlike [`Wallet::join`], the mnemonic is NOT written to
    /// `data_dir/mnemonic.txt` — the caller is responsible for keeping it
    /// (the desktop app stores it in the OS keychain). The mnemonic entropy
    /// is still stored inside the client database, mirroring fedimint-cli.
    /// A failed join removes the client database again so the directory is
    /// retryable.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::AlreadyInitialized`] if a wallet already exists
    /// at `data_dir` (client database or `mnemonic.txt` present),
    /// [`WalletError::InvalidInviteCode`] if the invite code fails to parse,
    /// [`WalletError::Mnemonic`] if `mnemonic` fails to parse, and
    /// [`WalletError::Federation`] if config download or the join itself
    /// fails.
    pub async fn join_with_mnemonic(
        data_dir: impl AsRef<Path>,
        invite_code: &str,
        mnemonic: &str,
    ) -> Result<Self, WalletError> {
        let data_dir = data_dir.as_ref();
        let invite = parse_invite(invite_code)?;
        let mnemonic = parse_mnemonic(mnemonic)?;

        std::fs::create_dir_all(data_dir).map_err(|source| WalletError::DataDir {
            path: data_dir.to_path_buf(),
            source,
        })?;
        if mnemonic_path(data_dir).exists() || data_dir.join(CLIENT_DB_DIR).exists() {
            return Err(WalletError::AlreadyInitialized(data_dir.to_path_buf()));
        }

        Self::join_parsed(data_dir, &invite, &mnemonic).await
    }

    /// Shared fallible tail of [`Wallet::join`] and
    /// [`Wallet::join_with_mnemonic`]: run the actual join and, on failure,
    /// remove the half-initialized client database so a retry does not fail
    /// [`WalletError::AlreadyInitialized`].
    async fn join_parsed(
        data_dir: &Path,
        invite: &InviteCode,
        mnemonic: &Mnemonic,
    ) -> Result<Self, WalletError> {
        match join_inner(data_dir, invite, mnemonic).await {
            Ok(client) => Ok(Self { client }),
            Err(e) => {
                let _ = std::fs::remove_dir_all(data_dir.join(CLIENT_DB_DIR));
                // fedimint-rocksdb holds its lock in a sibling file, not
                // inside the db directory.
                let _ = std::fs::remove_file(data_dir.join(format!("{CLIENT_DB_DIR}.lock")));
                Err(e)
            }
        }
    }

    /// Reopen an existing wallet previously created with [`Wallet::join`].
    ///
    /// Reads the mnemonic from `data_dir/mnemonic.txt` and opens the stored
    /// client database.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::NotInitialized`] if no wallet exists at
    /// `data_dir`, [`WalletError::Mnemonic`] if the persisted mnemonic fails
    /// to parse, and [`WalletError::Federation`] if the client fails to open.
    pub async fn open(data_dir: impl AsRef<Path>) -> Result<Self, WalletError> {
        let data_dir = data_dir.as_ref();
        let mnemonic_file = mnemonic_path(data_dir);
        if !mnemonic_file.exists() || !data_dir.join(CLIENT_DB_DIR).exists() {
            return Err(WalletError::NotInitialized(data_dir.to_path_buf()));
        }

        let mnemonic = read_mnemonic_file(&mnemonic_file)?;
        Self::open_parsed(data_dir, &mnemonic).await
    }

    /// Reopen an existing wallet previously created with
    /// [`Wallet::join_with_mnemonic`], deriving the root secret from the
    /// caller-provided `mnemonic` instead of reading `data_dir/mnemonic.txt`.
    ///
    /// The presence of the client database is the initialized-check;
    /// `mnemonic.txt` is not required to exist (and is ignored if it does).
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::NotInitialized`] if no client database exists
    /// at `data_dir`, [`WalletError::Mnemonic`] if `mnemonic` fails to
    /// parse, and [`WalletError::Federation`] if the client fails to open.
    pub async fn open_with_mnemonic(
        data_dir: impl AsRef<Path>,
        mnemonic: &str,
    ) -> Result<Self, WalletError> {
        let data_dir = data_dir.as_ref();
        let mnemonic = parse_mnemonic(mnemonic)?;
        if !data_dir.join(CLIENT_DB_DIR).exists() {
            return Err(WalletError::NotInitialized(data_dir.to_path_buf()));
        }
        Self::open_parsed(data_dir, &mnemonic).await
    }

    /// Shared tail of [`Wallet::open`] and [`Wallet::open_with_mnemonic`]:
    /// open the stored client database with the given root-secret mnemonic.
    async fn open_parsed(data_dir: &Path, mnemonic: &Mnemonic) -> Result<Self, WalletError> {
        let db = open_database(data_dir).await?;
        let builder = client_builder().await?;
        let client = builder
            .open(connectors().await?, db, root_secret(mnemonic))
            .await
            .map(Arc::new)
            .map_err(WalletError::Federation)?;

        Ok(Self { client })
    }

    /// Total spendable e-cash balance in millisatoshis.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::Federation`] if the primary (mint) module is
    /// unavailable.
    pub async fn balance(&self) -> Result<u64, WalletError> {
        Ok(self
            .client
            .get_balance_for_unit(AmountUnit::BITCOIN)
            .await
            .map_err(WalletError::Federation)?
            .msats)
    }

    /// Hex-encoded id of the federation this wallet is joined to.
    #[must_use]
    pub fn federation_id(&self) -> String {
        self.client.federation_id().to_string()
    }

    /// Summary of the wallet: federation id, name, network, and balance.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::Federation`] if the balance cannot be read.
    pub async fn info(&self) -> Result<WalletInfo, WalletError> {
        let config = self.client.config().await;
        let name = config.global.federation_name().map(ToOwned::to_owned);
        // The lightning module config carries the federation's bitcoin
        // network; the on-chain wallet module is deliberately not registered.
        let network = self
            .client
            .get_first_module::<LightningClientModule>()
            .ok()
            .map(|ln| ln.cfg.network.to_string());
        Ok(WalletInfo {
            federation_id: self.federation_id(),
            name,
            network,
            balance_msat: self.balance().await?,
        })
    }

    /// Spend e-cash out of band: select notes worth at least `amount_msat`
    /// and serialize them for a recipient.
    ///
    /// If the recipient never reissues the notes, the client reclaims them
    /// after `timeout` (fedimint `try_cancel_after`). Note selection allows
    /// overpay when the exact amount cannot be represented with available
    /// denominations — [`SpendResult::amount_msat`] reports the actual value.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::Federation`] if the balance is insufficient or
    /// the mint module rejects the spend.
    pub async fn spend(
        &self,
        amount_msat: u64,
        timeout: Duration,
    ) -> Result<SpendResult, WalletError> {
        let mint = self
            .client
            .get_first_module::<MintClientModule>()
            .map_err(WalletError::Federation)?;
        let (operation_id, notes) = mint
            .spend_notes_with_selector(
                &SelectNotesWithAtleastAmount,
                Amount::from_msats(amount_msat),
                timeout,
                false,
                (),
            )
            .await
            .map_err(WalletError::Federation)?;
        Ok(SpendResult {
            notes: notes.to_string(),
            operation_id: operation_id.fmt_full().to_string(),
            amount_msat: notes.total_amount().msats,
        })
    }

    /// Receive e-cash out of band: parse `notes`, reissue them into this
    /// wallet, and wait for the federation to complete the reissuance.
    ///
    /// Returns the reissued amount in millisatoshis.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::InvalidNotes`] if `notes` fails to parse,
    /// [`WalletError::WrongFederation`] if the notes were issued by a
    /// different federation, and [`WalletError::ReissueFailed`] if the
    /// federation reports the reissue operation as failed.
    pub async fn reissue(&self, notes: &str) -> Result<u64, WalletError> {
        let oob_notes: OOBNotes = notes
            .trim()
            .parse()
            .map_err(|e| WalletError::InvalidNotes(format!("{e:#}")))?;

        let wallet_federation_id = self.client.federation_id();
        if oob_notes.federation_id_prefix() != wallet_federation_id.to_prefix() {
            return Err(WalletError::WrongFederation {
                notes_prefix: oob_notes.federation_id_prefix().to_string(),
                wallet_federation_id: wallet_federation_id.to_string(),
            });
        }

        let amount = oob_notes.total_amount();
        let mint = self
            .client
            .get_first_module::<MintClientModule>()
            .map_err(WalletError::Federation)?;
        let operation_id = mint
            .reissue_external_notes(oob_notes, ())
            .await
            .map_err(WalletError::Federation)?;

        // Await completion: the update stream ends after a terminal state.
        let mut updates = mint
            .subscribe_reissue_external_notes(operation_id)
            .await
            .map_err(WalletError::Federation)?
            .into_stream();
        while let Some(update) = updates.next().await {
            match update {
                ReissueExternalNotesState::Failed(reason) => {
                    return Err(WalletError::ReissueFailed(reason));
                }
                ReissueExternalNotesState::Done => break,
                ReissueExternalNotesState::Created | ReissueExternalNotesState::Issuing => {}
            }
        }

        Ok(amount.msats)
    }
}

/// Path of the mnemonic file under the wallet data dir.
fn mnemonic_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MNEMONIC_FILE)
}

/// Parse a federation invite code.
fn parse_invite(invite_code: &str) -> Result<InviteCode, WalletError> {
    invite_code
        .trim()
        .parse()
        .map_err(|e| WalletError::InvalidInviteCode(format!("{e:#}")))
}

/// Parse a BIP-39 mnemonic phrase. The error message reports the parse
/// failure only — never the phrase itself.
fn parse_mnemonic(mnemonic: &str) -> Result<Mnemonic, WalletError> {
    mnemonic
        .trim()
        .parse::<Mnemonic>()
        .map_err(|e| WalletError::Mnemonic(e.to_string()))
}

/// Fallible tail of [`Wallet::join`]: open the db, store the entropy, and
/// join the federation. The db handle is dropped on the error path before
/// the caller removes the db directory.
async fn join_inner(
    data_dir: &Path,
    invite: &InviteCode,
    mnemonic: &Mnemonic,
) -> Result<ClientHandleArc, WalletError> {
    let db = open_database(data_dir).await?;
    // Mirror fedimint-cli: also store the entropy in the client database
    // so standard fedimint tooling can open this wallet directory.
    Client::store_encodable_client_secret(&db, mnemonic.to_entropy())
        .await
        .map_err(WalletError::Federation)?;

    let builder = client_builder().await?;
    builder
        .preview(connectors().await?, invite)
        .await
        .map_err(WalletError::Federation)?
        .join(db, root_secret(mnemonic))
        .await
        .map(Arc::new)
        .map_err(WalletError::Federation)
}

/// Write the mnemonic to `path`, refusing to overwrite and restricting the
/// file to the current user (mode 0600) on unix.
fn write_mnemonic_file(path: &Path, mnemonic: &Mnemonic) -> Result<(), WalletError> {
    use std::io::Write as _;

    let map_io = |source: std::io::Error| WalletError::DataDir {
        path: path.to_path_buf(),
        source,
    };

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(map_io)?;
    writeln!(file, "{mnemonic}").map_err(map_io)?;
    file.sync_all().map_err(map_io)?;
    Ok(())
}

/// Read and parse the mnemonic file written by [`write_mnemonic_file`].
fn read_mnemonic_file(path: &Path) -> Result<Mnemonic, WalletError> {
    let contents = std::fs::read_to_string(path).map_err(|source| WalletError::DataDir {
        path: path.to_path_buf(),
        source,
    })?;
    contents
        .trim()
        .parse()
        .map_err(|e| WalletError::Mnemonic(format!("{path}: {e}", path = path.display())))
}

/// Open (or create) the rocksdb client database under the wallet data dir.
async fn open_database(data_dir: &Path) -> Result<Database, WalletError> {
    Ok(
        fedimint_rocksdb::RocksDb::build(data_dir.join(CLIENT_DB_DIR))
            .open()
            .await
            .map_err(WalletError::Federation)?
            .into(),
    )
}

/// Build the connector registry used to reach federation guardians.
async fn connectors() -> Result<ConnectorRegistry, WalletError> {
    ConnectorRegistry::build_from_client_defaults()
        .bind()
        .await
        .map_err(WalletError::Federation)
}

/// Derive the client root secret from the wallet mnemonic (fedimint-cli
/// compatible standard double derivation).
fn root_secret(mnemonic: &Mnemonic) -> RootSecret {
    RootSecret::StandardDoubleDerive(
        Bip39RootSecretStrategy::<MNEMONIC_WORD_COUNT>::to_root_secret(mnemonic),
    )
}

/// Client builder with the Buzz module set registered: mint (primary
/// e-cash), lightning, and lightning-v2. The on-chain wallet module is
/// deliberately excluded.
async fn client_builder() -> Result<ClientBuilder, WalletError> {
    let mut registry = ClientModuleInitRegistry::new();
    registry.attach(MintClientInit);
    registry.attach(LightningClientInit::default());
    registry.attach(fedimint_lnv2_client::LightningClientInit::default());

    let mut builder = Client::builder().await.map_err(WalletError::Federation)?;
    builder.with_module_inits(registry);
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use fedimint_core::config::FederationId;
    use fedimint_core::util::SafeUrl;
    use fedimint_core::PeerId;

    use super::*;

    /// A syntactically valid invite code that points nowhere — parses fine,
    /// so tests exercise the checks that run before any network I/O.
    fn offline_invite_code() -> String {
        let url = SafeUrl::parse("wss://invalid.example.com/").expect("static url parses");
        InviteCode::new(url, PeerId::from(0), FederationId::dummy(), None).to_string()
    }

    #[test]
    fn generate_mnemonic_yields_twelve_parseable_words() {
        let phrase = Wallet::generate_mnemonic().expect("generation succeeds");
        assert_eq!(phrase.split_whitespace().count(), 12);
        assert!(parse_mnemonic(&phrase).is_ok(), "round-trips through parse");
    }

    #[test]
    fn generate_mnemonic_is_not_deterministic() {
        let a = Wallet::generate_mnemonic().expect("generation succeeds");
        let b = Wallet::generate_mnemonic().expect("generation succeeds");
        assert_ne!(a, b, "two generated mnemonics must differ");
    }

    #[test]
    fn parse_mnemonic_rejects_garbage_without_echoing_it() {
        let secret_garbage = "correct horse battery staple";
        let err = parse_mnemonic(secret_garbage).expect_err("garbage must not parse");
        assert!(matches!(err, WalletError::Mnemonic(_)));
        assert!(
            !err.to_string().contains("horse"),
            "error must not echo the input phrase: {err}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn join_with_mnemonic_rejects_bad_invite_code() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mnemonic = Wallet::generate_mnemonic().expect("generation succeeds");
        let result =
            Wallet::join_with_mnemonic(dir.path().join("w"), "not-an-invite", &mnemonic).await;
        assert!(matches!(result, Err(WalletError::InvalidInviteCode(_))));
        assert!(
            !dir.path().join("w").exists(),
            "no wallet dir may be created for a bad invite"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn join_with_mnemonic_rejects_bad_mnemonic_before_touching_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result =
            Wallet::join_with_mnemonic(dir.path().join("w"), &offline_invite_code(), "twelve bad")
                .await;
        assert!(matches!(result, Err(WalletError::Mnemonic(_))));
        assert!(
            !dir.path().join("w").exists(),
            "no wallet dir may be created for a bad mnemonic"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn join_with_mnemonic_refuses_existing_client_db() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(CLIENT_DB_DIR)).expect("seed db dir");
        let mnemonic = Wallet::generate_mnemonic().expect("generation succeeds");
        let result =
            Wallet::join_with_mnemonic(dir.path(), &offline_invite_code(), &mnemonic).await;
        assert!(matches!(result, Err(WalletError::AlreadyInitialized(_))));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn join_with_mnemonic_refuses_existing_mnemonic_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(mnemonic_path(dir.path()), "placeholder\n").expect("seed mnemonic file");
        let mnemonic = Wallet::generate_mnemonic().expect("generation succeeds");
        let result =
            Wallet::join_with_mnemonic(dir.path(), &offline_invite_code(), &mnemonic).await;
        assert!(
            matches!(result, Err(WalletError::AlreadyInitialized(_))),
            "a file-based wallet in the dir must not be clobbered"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_with_mnemonic_requires_client_db() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mnemonic = Wallet::generate_mnemonic().expect("generation succeeds");
        let result = Wallet::open_with_mnemonic(dir.path(), &mnemonic).await;
        assert!(
            matches!(result, Err(WalletError::NotInitialized(_))),
            "an empty dir is not an initialized wallet"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_with_mnemonic_does_not_require_mnemonic_file() {
        // The db-dir existing is the initialized-check; a bad mnemonic must
        // surface as a Mnemonic error, not NotInitialized, proving the file
        // was never consulted.
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(CLIENT_DB_DIR)).expect("seed db dir");
        let result = Wallet::open_with_mnemonic(dir.path(), "not a mnemonic").await;
        assert!(matches!(result, Err(WalletError::Mnemonic(_))));
    }
}
