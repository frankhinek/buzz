//! `buzz wallet` — Fedimint e-cash wallet commands.
//!
//! Wallet commands run locally against a federation (not the Buzz relay):
//! no `BUZZ_PRIVATE_KEY` or relay connection is required. Wallet state lives
//! in a data directory resolved from `--data-dir`, the `BUZZ_WALLET_DIR` env
//! var, or the platform default (`<data-dir>/buzz/wallet`).

use std::path::PathBuf;
use std::time::Duration;

use buzz_ecash::{Wallet, WalletError};

use crate::error::CliError;

/// Resolve the wallet data directory: explicit flag/env value, else the
/// platform data dir (`~/Library/Application Support` on macOS,
/// `~/.local/share` on Linux) under `buzz/wallet`.
fn resolve_data_dir(data_dir: Option<PathBuf>) -> Result<PathBuf, CliError> {
    if let Some(dir) = data_dir {
        return Ok(dir);
    }
    let base = dirs::data_dir().ok_or_else(|| {
        CliError::Other(
            "could not determine platform data directory; pass --data-dir or set BUZZ_WALLET_DIR"
                .into(),
        )
    })?;
    Ok(base.join("buzz").join("wallet"))
}

/// Map wallet-core errors onto CLI error categories / exit codes.
fn map_wallet_error(e: WalletError) -> CliError {
    match e {
        WalletError::InvalidInviteCode(_)
        | WalletError::InvalidNotes(_)
        | WalletError::WrongFederation { .. }
        | WalletError::AlreadyInitialized(_) => CliError::Usage(e.to_string()),
        WalletError::NotInitialized(_) => CliError::NotFound(e.to_string()),
        WalletError::DataDir { .. } | WalletError::Mnemonic(_) => CliError::Other(e.to_string()),
        WalletError::ReissueFailed(reason) => CliError::Federation(format!("reissue: {reason}")),
        WalletError::Federation(inner) => CliError::Federation(format!("{inner:#}")),
    }
}

/// Open an existing wallet at the resolved data dir.
async fn open_wallet(data_dir: Option<PathBuf>) -> Result<Wallet, CliError> {
    let dir = resolve_data_dir(data_dir)?;
    Wallet::open(&dir).await.map_err(map_wallet_error)
}

/// `buzz wallet join` — create a wallet and join a federation.
pub async fn cmd_join(invite_code: &str, data_dir: Option<PathBuf>) -> Result<(), CliError> {
    let dir = resolve_data_dir(data_dir)?;
    let wallet = Wallet::join(&dir, invite_code)
        .await
        .map_err(map_wallet_error)?;
    let info = wallet.info().await.map_err(map_wallet_error)?;
    let out = serde_json::json!({
        "federation_id": info.federation_id,
        "name": info.name,
    });
    println!("{out}");
    Ok(())
}

/// `buzz wallet balance` — spendable e-cash balance in millisatoshis.
pub async fn cmd_balance(data_dir: Option<PathBuf>) -> Result<(), CliError> {
    let wallet = open_wallet(data_dir).await?;
    let balance_msat = wallet.balance().await.map_err(map_wallet_error)?;
    println!("{}", serde_json::json!({ "balance_msat": balance_msat }));
    Ok(())
}

/// `buzz wallet spend` — prepare out-of-band notes for a recipient.
pub async fn cmd_spend(
    amount_msat: u64,
    timeout_secs: u64,
    data_dir: Option<PathBuf>,
) -> Result<(), CliError> {
    if amount_msat == 0 {
        return Err(CliError::Usage(
            "amount_msat must be greater than zero".into(),
        ));
    }
    let wallet = open_wallet(data_dir).await?;
    let result = wallet
        .spend(amount_msat, Duration::from_secs(timeout_secs))
        .await
        .map_err(map_wallet_error)?;
    let out = serde_json::json!({
        "notes": result.notes,
        "amount_msat": result.amount_msat,
        "operation_id": result.operation_id,
    });
    println!("{out}");
    Ok(())
}

/// `buzz wallet reissue` — redeem received out-of-band notes into the wallet.
pub async fn cmd_reissue(notes: &str, data_dir: Option<PathBuf>) -> Result<(), CliError> {
    let wallet = open_wallet(data_dir).await?;
    let amount_msat = wallet.reissue(notes).await.map_err(map_wallet_error)?;
    println!("{}", serde_json::json!({ "amount_msat": amount_msat }));
    Ok(())
}

/// `buzz wallet info` — federation id, name, network, and balance.
pub async fn cmd_info(data_dir: Option<PathBuf>) -> Result<(), CliError> {
    let wallet = open_wallet(data_dir).await?;
    let info = wallet.info().await.map_err(map_wallet_error)?;
    println!(
        "{}",
        serde_json::to_string(&info)
            .map_err(|e| CliError::Other(format!("failed to serialize wallet info: {e}")))?
    );
    Ok(())
}

/// Dispatch a `buzz wallet` subcommand.
pub async fn dispatch(cmd: &crate::WalletCmd) -> Result<(), CliError> {
    use crate::WalletCmd;
    match cmd {
        WalletCmd::Join {
            invite_code,
            data_dir,
        } => cmd_join(invite_code, data_dir.clone()).await,
        WalletCmd::Balance { data_dir } => cmd_balance(data_dir.clone()).await,
        WalletCmd::Spend {
            amount_msat,
            timeout_secs,
            data_dir,
        } => cmd_spend(*amount_msat, *timeout_secs, data_dir.clone()).await,
        WalletCmd::Reissue { notes, data_dir } => cmd_reissue(notes, data_dir.clone()).await,
        WalletCmd::Info { data_dir } => cmd_info(data_dir.clone()).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_data_dir_wins() {
        let dir = resolve_data_dir(Some(PathBuf::from("/tmp/custom-wallet"))).unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/custom-wallet"));
    }

    #[test]
    fn default_data_dir_is_under_platform_data_dir() {
        let dir = resolve_data_dir(None).unwrap();
        assert!(
            dir.ends_with("buzz/wallet"),
            "default should end with buzz/wallet: {}",
            dir.display()
        );
    }

    #[test]
    fn wallet_errors_map_to_cli_categories() {
        assert!(matches!(
            map_wallet_error(WalletError::InvalidInviteCode("bad".into())),
            CliError::Usage(_)
        ));
        assert!(matches!(
            map_wallet_error(WalletError::InvalidNotes("bad".into())),
            CliError::Usage(_)
        ));
        assert!(matches!(
            map_wallet_error(WalletError::NotInitialized("/nope".into())),
            CliError::NotFound(_)
        ));
        assert!(matches!(
            map_wallet_error(WalletError::AlreadyInitialized("/dup".into())),
            CliError::Usage(_)
        ));
        assert!(matches!(
            map_wallet_error(WalletError::ReissueFailed("rejected".into())),
            CliError::Federation(_)
        ));
        assert!(matches!(
            map_wallet_error(WalletError::Federation(anyhow::anyhow!("offline"))),
            CliError::Federation(_)
        ));
    }
}
