//! Fedimint e-cash wallet core for Buzz.
//!
//! Phase 0 spike: proves the fedimint client stack resolves and compiles
//! inside the Buzz workspace. Real wallet operations (join, balance, spend,
//! reissue) land in Phase 1. See `wiki/integration-design.md`.

use fedimint_core::invite_code::InviteCode;

/// Handle type Buzz wallet code will hold once a client is opened (Phase 1).
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
}
