//! Fedimint e-cash wallet commands (Phase 2: one wallet per app).
//!
//! Wallet data lives under `<app_data_dir>/ecash/<federation_id>/` (rocksdb
//! client database). The BIP-39 mnemonic that derives the wallet's root
//! secret is NOT stored on disk — it lives in the OS keyring blob managed by
//! [`crate::secret_store::SecretStore`], under the key
//! `ecash_mnemonic:<federation_id>`. Join order is keyring-first: the
//! mnemonic is persisted before the federation join starts, and removed
//! again if the join fails, so a crash can strand at worst an unused keyring
//! entry — never wallet funds without their secret.
//!
//! Phase 2 supports exactly ONE wallet per app installation. The single
//! source of truth is the `ecash/` directory: if any wallet dir exists,
//! [`wallet_join`] for a different federation is refused with an error that
//! names the existing federation.
//!
//! Concurrency: the open wallet handle lives in [`WalletManager`] behind a
//! `tokio::sync::Mutex`, held across the whole open/join so concurrent
//! commands serialize instead of double-opening the rocksdb dir. The
//! fedimint client requires a multi-thread tokio runtime (rocksdb and client
//! shutdown block in place); Tauri's async runtime qualifies.

use std::path::{Path, PathBuf};
use std::time::Duration;

use buzz_ecash_pkg::{Wallet, WalletInfo};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::app_state::keyring_service;
use crate::secret_store::SecretStore;

/// Directory under the Tauri app data dir holding per-federation wallets.
const ECASH_DIR: &str = "ecash";

/// Default out-of-band spend timeout (fedimint `try_cancel_after`) when the
/// frontend does not pass one: 24 hours.
const DEFAULT_SPEND_TIMEOUT_SECS: u64 = 86_400;

/// Managed Tauri state owning the (at most one) open wallet handle.
///
/// Community switching note: the frontend remounts on community switch but
/// Rust managed state persists, so the same wallet handle survives the
/// switch. That is acceptable for Phase 2 (one wallet per app, not per
/// community); the frontend may call [`wallet_lock`] on community switch to
/// drop the handle. Per-community wallet selection is future work.
pub struct WalletManager {
    wallet: tokio::sync::Mutex<Option<Wallet>>,
}

impl WalletManager {
    /// Fresh manager with no wallet open.
    pub fn new() -> Self {
        Self {
            wallet: tokio::sync::Mutex::new(None),
        }
    }
}

impl Default for WalletManager {
    fn default() -> Self {
        Self::new()
    }
}

/// The keyring operations the wallet flow needs. Abstracted (mirroring
/// `IdentityKeyStore` in `app_state.rs`) so join/cleanup logic can be
/// unit-tested against an in-memory fake without touching the OS keychain.
trait MnemonicStore: Send + Sync {
    fn load(&self, key: &str) -> Result<Option<String>, String>;
    fn store(&self, key: &str, value: &str) -> Result<(), String>;
    fn delete(&self, key: &str) -> Result<(), String>;
}

impl MnemonicStore for SecretStore {
    fn load(&self, key: &str) -> Result<Option<String>, String> {
        SecretStore::load(self, key)
    }
    fn store(&self, key: &str, value: &str) -> Result<(), String> {
        SecretStore::store(self, key, value)
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        SecretStore::delete(self, key)
    }
}

/// Keyring blob key for a federation's wallet mnemonic. Namespaced with an
/// `ecash_mnemonic:` prefix so it can never collide with the `identity` /
/// agent nsec entries sharing the same blob.
fn mnemonic_key(federation_id: &str) -> String {
    format!("ecash_mnemonic:{federation_id}")
}

/// `<base>/ecash` — the root holding one subdirectory per joined federation.
fn ecash_root_from(base: &Path) -> PathBuf {
    base.join(ECASH_DIR)
}

/// `<ecash_root>/<federation_id>` — one wallet's data dir.
fn wallet_data_dir(ecash_root: &Path, federation_id: &str) -> PathBuf {
    ecash_root.join(federation_id)
}

/// Resolve the ecash root under the Tauri app data dir.
fn ecash_root(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
    Ok(ecash_root_from(&base))
}

/// Scan the ecash root for an existing wallet and return its federation id
/// (the subdirectory name). Plain files are ignored. Phase 2 maintains at
/// most one wallet dir; if several exist (e.g. hand-copied), the
/// lexicographically first is authoritative so the choice is deterministic.
fn existing_wallet_federation(root: &Path) -> Result<Option<String>, String> {
    if !root.exists() {
        return Ok(None);
    }
    let entries =
        std::fs::read_dir(root).map_err(|e| format!("failed to read {}: {e}", root.display()))?;
    let mut federations: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    federations.sort();
    Ok(federations.into_iter().next())
}

/// Error for a join refused by the single-wallet guard. Always names the
/// existing federation so the frontend can render an actionable message.
fn single_wallet_conflict(existing: &str, requested: &str) -> String {
    if existing == requested {
        format!("already joined federation {existing}")
    } else {
        format!(
            "a wallet for federation {existing} already exists; \
             this app supports a single e-cash wallet, so federation \
             {requested} cannot be joined"
        )
    }
}

/// Roll back a failed join: remove the keyring mnemonic entry (no orphaned
/// secrets) and the wallet data dir. Best-effort — the dir did not exist
/// before the join attempt (single-wallet guard), so removal is safe.
fn cleanup_failed_join(store: &dyn MnemonicStore, key: &str, data_dir: &Path) {
    let _ = store.delete(key);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// The process-wide keyring store, shared with identity/agent secrets.
fn secret_store() -> &'static SecretStore {
    SecretStore::shared(keyring_service())
}

/// Open the on-disk wallet into `slot` if it is not already open.
///
/// Returns `Ok(false)` when no wallet exists on this machine (nothing under
/// `ecash/`), `Ok(true)` when a wallet is (now) open. A wallet dir whose
/// mnemonic is missing from the keyring is an error, not "none" — the funds
/// exist but are locked, and the frontend should surface that.
async fn open_if_needed(app: &AppHandle, slot: &mut Option<Wallet>) -> Result<bool, String> {
    if slot.is_some() {
        return Ok(true);
    }
    let root = ecash_root(app)?;
    let Some(federation_id) = existing_wallet_federation(&root)? else {
        return Ok(false);
    };
    let mnemonic =
        MnemonicStore::load(secret_store(), &mnemonic_key(&federation_id))?.ok_or_else(|| {
            format!(
                "wallet data exists for federation {federation_id} \
                 but its mnemonic is missing from the OS keyring"
            )
        })?;
    let wallet = Wallet::open_with_mnemonic(wallet_data_dir(&root, &federation_id), &mnemonic)
        .await
        .map_err(|e| e.to_string())?;
    *slot = Some(wallet);
    Ok(true)
}

/// `wallet_status` response. `state` is `"none"` (no wallet on this machine)
/// or `"open"`; the remaining fields are present only when open.
#[derive(Debug, Serialize)]
pub struct EcashWalletStatus {
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    federation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    balance_msat: Option<u64>,
}

impl EcashWalletStatus {
    fn none() -> Self {
        Self {
            state: "none",
            federation_id: None,
            name: None,
            network: None,
            balance_msat: None,
        }
    }

    fn open(info: WalletInfo) -> Self {
        Self {
            state: "open",
            federation_id: Some(info.federation_id),
            name: info.name,
            network: info.network,
            balance_msat: Some(info.balance_msat),
        }
    }
}

/// `wallet_balance` response.
#[derive(Debug, Serialize)]
pub struct EcashWalletBalance {
    balance_msat: u64,
}

/// `wallet_spend` response: serialized out-of-band notes for the recipient.
#[derive(Debug, Serialize)]
pub struct EcashWalletSpend {
    notes: String,
    amount_msat: u64,
    operation_id: String,
}

/// `wallet_reissue` response: the reissued amount and the post-reissue balance.
#[derive(Debug, Serialize)]
pub struct EcashWalletReissue {
    amount_msat: u64,
    balance_msat: u64,
}

/// Current wallet state. Lazily auto-opens the wallet on first call when a
/// data dir and keyring mnemonic exist; subsequent calls reuse the handle.
#[tauri::command]
pub async fn wallet_status(
    app: AppHandle,
    manager: State<'_, WalletManager>,
) -> Result<EcashWalletStatus, String> {
    let mut guard = manager.wallet.lock().await;
    if !open_if_needed(&app, &mut guard).await? {
        return Ok(EcashWalletStatus::none());
    }
    let Some(wallet) = guard.as_ref() else {
        return Ok(EcashWalletStatus::none());
    };
    let info = wallet.info().await.map_err(|e| e.to_string())?;
    Ok(EcashWalletStatus::open(info))
}

/// Join a federation from an invite code, creating this machine's single
/// e-cash wallet.
///
/// Order of operations: derive the federation id from the invite (offline),
/// refuse if any wallet already exists, generate a mnemonic, persist it to
/// the OS keyring FIRST, then join. A failed join deletes the keyring entry
/// and the data dir again, so retries start clean.
#[tauri::command]
pub async fn wallet_join(
    invite_code: String,
    app: AppHandle,
    manager: State<'_, WalletManager>,
) -> Result<WalletInfo, String> {
    let federation_id = buzz_ecash_pkg::federation_id_from_invite(&invite_code)
        .map_err(|e| format!("invalid invite code: {e}"))?;

    // Hold the manager lock across the whole join: serializes concurrent
    // join/status calls and keeps the single-wallet guard race-free.
    let mut guard = manager.wallet.lock().await;

    let root = ecash_root(&app)?;
    if let Some(existing) = existing_wallet_federation(&root)? {
        return Err(single_wallet_conflict(&existing, &federation_id));
    }

    let store = secret_store();
    let key = mnemonic_key(&federation_id);
    let mnemonic = Wallet::generate_mnemonic().map_err(|e| e.to_string())?;
    // Keyring first: if the process dies mid-join the secret already exists,
    // so recovery can never end up with a joined wallet missing its mnemonic.
    MnemonicStore::store(store, &key, &mnemonic)?;

    let data_dir = wallet_data_dir(&root, &federation_id);
    match Wallet::join_with_mnemonic(&data_dir, &invite_code, &mnemonic).await {
        Ok(wallet) => {
            let wallet = guard.insert(wallet);
            wallet.info().await.map_err(|e| e.to_string())
        }
        Err(e) => {
            cleanup_failed_join(store, &key, &data_dir);
            Err(e.to_string())
        }
    }
}

/// Spendable e-cash balance in millisatoshis.
#[tauri::command]
pub async fn wallet_balance(
    app: AppHandle,
    manager: State<'_, WalletManager>,
) -> Result<EcashWalletBalance, String> {
    let mut guard = manager.wallet.lock().await;
    let wallet = require_open(&app, &mut guard).await?;
    let balance_msat = wallet.balance().await.map_err(|e| e.to_string())?;
    Ok(EcashWalletBalance { balance_msat })
}

/// Wallet summary: federation id, name, network, and balance.
#[tauri::command]
pub async fn wallet_info(
    app: AppHandle,
    manager: State<'_, WalletManager>,
) -> Result<WalletInfo, String> {
    let mut guard = manager.wallet.lock().await;
    let wallet = require_open(&app, &mut guard).await?;
    wallet.info().await.map_err(|e| e.to_string())
}

/// Spend e-cash out of band: returns serialized notes for a recipient. If
/// the notes are never reissued, the wallet reclaims them after
/// `timeout_secs` (default 24h). `amount_msat` in the response is the actual
/// note value, which may exceed the request when exact change was
/// unavailable.
#[tauri::command]
pub async fn wallet_spend(
    amount_msat: u64,
    timeout_secs: Option<u64>,
    app: AppHandle,
    manager: State<'_, WalletManager>,
) -> Result<EcashWalletSpend, String> {
    if amount_msat == 0 {
        return Err("amount_msat must be greater than zero".to_string());
    }
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_SPEND_TIMEOUT_SECS));
    let mut guard = manager.wallet.lock().await;
    let wallet = require_open(&app, &mut guard).await?;
    let result = wallet
        .spend(amount_msat, timeout)
        .await
        .map_err(|e| e.to_string())?;
    Ok(EcashWalletSpend {
        notes: result.notes,
        amount_msat: result.amount_msat,
        operation_id: result.operation_id,
    })
}

/// Receive e-cash out of band: reissue `notes` into this wallet and wait for
/// the federation to complete. Returns the reissued amount and the
/// post-reissue balance.
#[tauri::command]
pub async fn wallet_reissue(
    notes: String,
    app: AppHandle,
    manager: State<'_, WalletManager>,
) -> Result<EcashWalletReissue, String> {
    let mut guard = manager.wallet.lock().await;
    let wallet = require_open(&app, &mut guard).await?;
    let amount_msat = wallet.reissue(&notes).await.map_err(|e| e.to_string())?;
    let balance_msat = wallet.balance().await.map_err(|e| e.to_string())?;
    Ok(EcashWalletReissue {
        amount_msat,
        balance_msat,
    })
}

/// Drop the open wallet handle (shuts the fedimint client down). The wallet
/// reopens lazily on the next command. The frontend may call this on
/// community switch; per-community wallet selection is future work — for
/// Phase 2 the single wallet is app-wide, so locking is optional hygiene.
#[tauri::command]
pub async fn wallet_lock(manager: State<'_, WalletManager>) -> Result<(), String> {
    let mut guard = manager.wallet.lock().await;
    *guard = None;
    Ok(())
}

/// Auto-open like [`open_if_needed`], but "no wallet" is an error — used by
/// the commands that require a joined wallet.
async fn require_open<'a>(
    app: &AppHandle,
    slot: &'a mut Option<Wallet>,
) -> Result<&'a Wallet, String> {
    if !open_if_needed(app, slot).await? {
        return Err("no e-cash wallet on this machine — join a federation first".to_string());
    }
    slot.as_ref()
        .ok_or_else(|| "wallet failed to open".to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    /// In-memory [`MnemonicStore`] so tests never touch the OS keychain.
    #[derive(Default)]
    struct FakeStore {
        entries: Mutex<HashMap<String, String>>,
    }

    impl MnemonicStore for FakeStore {
        fn load(&self, key: &str) -> Result<Option<String>, String> {
            Ok(self
                .entries
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(key)
                .cloned())
        }
        fn store(&self, key: &str, value: &str) -> Result<(), String> {
            self.entries
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn delete(&self, key: &str) -> Result<(), String> {
            self.entries
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(key);
            Ok(())
        }
    }

    #[test]
    fn mnemonic_key_is_namespaced_per_federation() {
        let key = mnemonic_key("f00dbabe");
        assert_eq!(key, "ecash_mnemonic:f00dbabe");
        assert_ne!(
            mnemonic_key("aaa"),
            mnemonic_key("bbb"),
            "distinct federations must map to distinct keyring keys"
        );
        assert_ne!(key, "identity", "must never collide with the identity key");
    }

    #[test]
    fn wallet_dirs_nest_under_ecash_root() {
        let base = Path::new("/data/app");
        let root = ecash_root_from(base);
        assert_eq!(root, Path::new("/data/app/ecash"));
        assert_eq!(
            wallet_data_dir(&root, "f00d"),
            Path::new("/data/app/ecash/f00d")
        );
    }

    #[test]
    fn existing_wallet_federation_handles_missing_and_empty_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nope");
        assert_eq!(existing_wallet_federation(&missing).unwrap(), None);
        assert_eq!(existing_wallet_federation(dir.path()).unwrap(), None);
    }

    #[test]
    fn existing_wallet_federation_finds_wallet_dir_and_ignores_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("stray.txt"), "not a wallet").unwrap();
        assert_eq!(
            existing_wallet_federation(dir.path()).unwrap(),
            None,
            "plain files are not wallets"
        );
        std::fs::create_dir(dir.path().join("feddeadbeef")).unwrap();
        assert_eq!(
            existing_wallet_federation(dir.path()).unwrap(),
            Some("feddeadbeef".to_string())
        );
    }

    #[test]
    fn existing_wallet_federation_is_deterministic_across_multiple_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("bbb")).unwrap();
        std::fs::create_dir(dir.path().join("aaa")).unwrap();
        assert_eq!(
            existing_wallet_federation(dir.path()).unwrap(),
            Some("aaa".to_string()),
            "lexicographically first dir wins"
        );
    }

    #[test]
    fn single_wallet_guard_names_the_existing_federation() {
        let msg = single_wallet_conflict("fed_old", "fed_new");
        assert!(msg.contains("fed_old"), "must name the existing federation");
        assert!(msg.contains("fed_new"), "must name the refused federation");

        let same = single_wallet_conflict("fed_old", "fed_old");
        assert!(same.contains("already joined"));
        assert!(same.contains("fed_old"));
    }

    #[test]
    fn mnemonic_round_trips_through_the_store_seam() {
        let store = FakeStore::default();
        let key = mnemonic_key("cafef00d");
        let mnemonic =
            "abandon ability able about above absent absorb abstract absurd abuse access accident";
        MnemonicStore::store(&store, &key, mnemonic).unwrap();
        assert_eq!(
            MnemonicStore::load(&store, &key).unwrap().as_deref(),
            Some(mnemonic)
        );
        MnemonicStore::delete(&store, &key).unwrap();
        assert_eq!(MnemonicStore::load(&store, &key).unwrap(), None);
    }

    #[test]
    fn cleanup_failed_join_removes_secret_and_data_dir() {
        let store = FakeStore::default();
        let key = mnemonic_key("cafef00d");
        MnemonicStore::store(&store, &key, "some mnemonic").unwrap();

        let base = tempfile::tempdir().expect("tempdir");
        let root = ecash_root_from(base.path());
        let data_dir = wallet_data_dir(&root, "cafef00d");
        std::fs::create_dir_all(data_dir.join("client.db")).unwrap();

        cleanup_failed_join(&store, &key, &data_dir);

        assert_eq!(
            MnemonicStore::load(&store, &key).unwrap(),
            None,
            "no orphaned keyring secret after a failed join"
        );
        assert!(!data_dir.exists(), "wallet dir removed after a failed join");
        assert_eq!(
            existing_wallet_federation(&root).unwrap(),
            None,
            "guard sees a clean slate after cleanup"
        );
    }

    #[test]
    fn cleanup_failed_join_tolerates_already_missing_state() {
        let store = FakeStore::default();
        let base = tempfile::tempdir().expect("tempdir");
        // Nothing stored, nothing on disk — must not panic or error.
        cleanup_failed_join(
            &store,
            &mnemonic_key("cafef00d"),
            &wallet_data_dir(&ecash_root_from(base.path()), "cafef00d"),
        );
    }
}
