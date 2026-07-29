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

/// Proves the desktop keychain flow against a live federation: generate a
/// mnemonic externally, join with it (no `mnemonic.txt` written), reopen
/// with the same mnemonic, and confirm the file-based `open` refuses the
/// directory (there is no mnemonic file to read).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires FM_INVITE_CODE from a running federation"]
async fn mnemonic_injection_round_trip_against_live_federation() {
    let invite = std::env::var("FM_INVITE_CODE").expect("set FM_INVITE_CODE");
    let dir = tempfile::tempdir().expect("tempdir");

    let mnemonic = Wallet::generate_mnemonic().expect("generate mnemonic");
    let wallet = Wallet::join_with_mnemonic(dir.path(), &invite, &mnemonic)
        .await
        .expect("join_with_mnemonic should succeed against a live federation");
    let federation_id = wallet.federation_id();
    assert_eq!(federation_id.len(), 64, "federation id is 32 hex bytes");
    assert_eq!(wallet.balance().await.expect("balance"), 0);
    assert!(
        !dir.path().join("mnemonic.txt").exists(),
        "join_with_mnemonic must not write mnemonic.txt"
    );

    // Joining the same data dir twice must fail regardless of flow.
    let second = Wallet::join_with_mnemonic(dir.path(), &invite, &mnemonic).await;
    assert!(matches!(second, Err(WalletError::AlreadyInitialized(_))));

    drop(wallet);

    // The file-based open must refuse the dir — there is no mnemonic.txt.
    let file_open = Wallet::open(dir.path()).await;
    assert!(
        matches!(file_open, Err(WalletError::NotInitialized(_))),
        "file-based open must not open a keychain-flow wallet"
    );

    // Reopen with the injected mnemonic.
    let wallet = Wallet::open_with_mnemonic(dir.path(), &mnemonic)
        .await
        .expect("open_with_mnemonic should reopen the wallet");
    assert_eq!(wallet.federation_id(), federation_id);
    assert_eq!(wallet.balance().await.expect("balance"), 0);

    // Optionally exercise real funds through the injected-mnemonic wallet.
    // Uses a dedicated env var — FM_FUNDED_NOTES belongs to the file-based
    // test in this binary, and out-of-band notes are single-use.
    if let Ok(notes) = std::env::var("FM_FUNDED_NOTES_MNEMONIC") {
        let received = wallet.reissue(notes.trim()).await.expect("reissue");
        assert!(received > 0);
        let spend = wallet
            .spend(received / 2, Duration::from_secs(3600))
            .await
            .expect("spend");
        assert!(!spend.notes.is_empty());
    }
}

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
