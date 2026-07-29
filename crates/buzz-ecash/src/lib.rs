//! Fedimint e-cash wallet core for Buzz.
//!
//! Wraps the fedimint client stack (v0.11) in a small async [`Wallet`] API:
//! join a federation from an invite code, reopen an existing wallet, check
//! the balance, spend e-cash out of band, and reissue received notes. See
//! `wiki/integration-design.md` for the integration plan.

mod error;
mod wallet;

pub use error::WalletError;
pub use wallet::{SpendResult, Wallet, WalletInfo};

use fedimint_core::invite_code::InviteCode;

/// Handle type Buzz wallet code holds once a client is opened.
pub type WalletClient = fedimint_client::ClientHandleArc;

/// Parse a bech32 `fed1…` federation invite code and return the federation
/// id as a hex string.
///
/// # Errors
///
/// Returns the parse error message if `code` is not a valid invite code.
pub fn federation_id_from_invite(code: &str) -> Result<String, String> {
    let invite: InviteCode = code.parse().map_err(|e| format!("{e:#}"))?;
    Ok(invite.federation_id().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage_invite_code() {
        assert!(federation_id_from_invite("not-an-invite-code").is_err());
    }

    /// Validates a real invite code from a devimint regtest federation.
    /// Boot one with `just mprocs` (or `devimint dev-fed`) in a fedimint
    /// checkout, then run:
    /// `FM_INVITE_CODE=fed1... cargo test -p buzz-ecash -- --ignored`
    #[test]
    #[ignore = "requires FM_INVITE_CODE from a devimint federation"]
    fn parses_devimint_invite_code() {
        let code = std::env::var("FM_INVITE_CODE").expect("set FM_INVITE_CODE");
        let federation_id =
            federation_id_from_invite(code.trim()).expect("devimint invite code should parse");
        assert_eq!(
            federation_id.len(),
            64,
            "federation id should be 32 hex-encoded bytes, got: {federation_id}"
        );
        // Cross-check against the id the federation reports about itself
        // (e.g. `fedimint-cli info | jq -r .federation_id`), when provided.
        if let Ok(expected) = std::env::var("FM_EXPECTED_FEDERATION_ID") {
            assert_eq!(federation_id, expected.trim());
        }
    }
}
