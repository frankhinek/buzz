//! Error types for wallet operations.

use std::path::PathBuf;

use thiserror::Error;

/// Errors returned by [`crate::Wallet`] operations.
#[derive(Debug, Error)]
pub enum WalletError {
    /// The federation invite code failed to parse.
    #[error("invalid invite code: {0}")]
    InvalidInviteCode(String),

    /// The out-of-band e-cash notes string failed to parse.
    #[error("invalid e-cash notes: {0}")]
    InvalidNotes(String),

    /// The e-cash notes belong to a different federation than the wallet.
    #[error(
        "e-cash notes belong to federation prefix {notes_prefix}, \
         wallet is joined to {wallet_federation_id}"
    )]
    WrongFederation {
        /// Federation id prefix encoded in the notes.
        notes_prefix: String,
        /// Full federation id of this wallet.
        wallet_federation_id: String,
    },

    /// Filesystem error under the wallet data directory.
    #[error("wallet data dir error at {path}: {source}")]
    DataDir {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// No wallet exists at the data directory.
    #[error("no wallet found at {} — run join first", .0.display())]
    NotInitialized(PathBuf),

    /// A wallet already exists at the data directory.
    #[error("wallet already exists at {}", .0.display())]
    AlreadyInitialized(PathBuf),

    /// Mnemonic generation or parsing failed.
    #[error("mnemonic error: {0}")]
    Mnemonic(String),

    /// The federation reported the reissue operation as failed.
    #[error("reissue failed: {0}")]
    ReissueFailed(String),

    /// Any other failure from the fedimint client stack (config download,
    /// network, consensus, or module errors). Renders the full cause chain.
    #[error("federation error: {0:#}")]
    Federation(anyhow::Error),
}

impl From<anyhow::Error> for WalletError {
    fn from(e: anyhow::Error) -> Self {
        Self::Federation(e)
    }
}
