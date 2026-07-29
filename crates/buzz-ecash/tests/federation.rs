//! Live-federation integration test for the e-cash wallet.
//!
//! Requires a running federation (e.g. `devimint dev-fed` in a fedimint
//! checkout). Run with:
//!
//! ```text
//! FM_INVITE_CODE=fed1... cargo test -p buzz-ecash --test federation -- --ignored
//! ```
//!
//! Optionally set `FM_FUNDED_NOTES` to a serialized out-of-band notes string
//! (e.g. from `fedimint-cli spend 100000 | jq -r .notes`) to also exercise
//! reissue + spend against real funds.

use std::time::Duration;

use buzz_ecash::{Wallet, WalletError};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires FM_INVITE_CODE from a running federation"]
async fn join_balance_reissue_against_live_federation() {
    let invite = std::env::var("FM_INVITE_CODE").expect("set FM_INVITE_CODE");
    let dir = tempfile::tempdir().expect("tempdir");

    // Join creates the wallet and persists the mnemonic.
    let wallet = Wallet::join(dir.path(), &invite)
        .await
        .expect("join should succeed against a live federation");
    let federation_id = wallet.federation_id();
    assert_eq!(federation_id.len(), 64, "federation id is 32 hex bytes");
    assert_eq!(wallet.balance().await.expect("balance"), 0);

    // Joining the same data dir twice must fail.
    let second_join = Wallet::join(dir.path(), &invite).await;
    assert!(
        matches!(second_join, Err(WalletError::AlreadyInitialized(_))),
        "second join into the same dir should fail"
    );

    let info = wallet.info().await.expect("info");
    assert_eq!(info.federation_id, federation_id);
    assert_eq!(info.balance_msat, 0);

    // Reopen the wallet from disk.
    drop(wallet);
    let wallet = Wallet::open(dir.path())
        .await
        .expect("open should succeed after join");
    assert_eq!(wallet.federation_id(), federation_id);
    assert_eq!(wallet.balance().await.expect("balance"), 0);

    // With real funds provided: reissue them, then spend a part back out.
    if let Ok(notes) = std::env::var("FM_FUNDED_NOTES") {
        let received = wallet
            .reissue(notes.trim())
            .await
            .expect("reissue of funded notes should succeed");
        assert!(received > 0, "reissued amount should be positive");

        let balance = wallet.balance().await.expect("balance");
        assert!(
            balance > 0,
            "balance should be positive after reissue, got {balance}"
        );

        let spend_amount = balance / 2;
        let spend = wallet
            .spend(spend_amount, Duration::from_secs(3600))
            .await
            .expect("spend should succeed with sufficient balance");
        assert!(spend.amount_msat >= spend_amount);
        assert!(!spend.notes.is_empty());

        let after = wallet.balance().await.expect("balance");
        assert!(
            after < balance,
            "balance should decrease after spend: before={balance} after={after}"
        );
    }
}
