# Integration Design

Proposed architecture for Fedimint e-cash in Buzz. Nothing implemented yet. Decision record in [project.md](project.md#decisions).

## Principles

- **Wallets live on devices, never on the relay.** The relay transports opaque encrypted events. It never sees notes, balances, or the client database.
- **Community = federation.** Fedimint's trust model (a community trusting a set of guardians) is exactly Buzz's model of a self-hosted community. Same-federation-only notes stop being a limitation when the community is the federation.
- **One Rust core, multiple shells.** Same shape as the Fedi bridge and Buzz's own desktop backend ([fedimint.md](fedimint.md#the-fedi-bridge-architecture-reference)).

## Components

**`buzz-ecash` crate** (new, in the root workspace): wraps `fedimint-client` + `fedimint-mint-client` (primary) + `fedimint-lnv2-client`/`fedimint-ln-client` + `fedimint-bip39`, storage via `fedimint-rocksdb` in per-device app data. Exposes join/balance/spend/reissue/subscribe operations.

Hosts:

- **Desktop**: link `buzz-ecash` from `desktop/src-tauri`, expose `#[tauri::command]` wallet ops, seed in the existing keychain-backed `SecretStore`, client DB next to `retention.db`. Must compile under the separate src-tauri workspace resolution ([buzz.md](buzz.md#clients)).
- **CLI**: `buzz wallet` subcommands in `buzz-cli`. Seed via env/config, consistent with `BUZZ_PRIVATE_KEY` handling.
- **Mobile (later)**: flutter_rust_bridge over the same crate, following `fedimint/ecash-app`. Alternative: Fedi's UniFFI string-RPC pattern, coarser but proven and portable to any future shell.

**Payment flow (DM)**: sender's client calls `spend_notes(try_cancel_after: generous)`, wraps the OOBNotes string + metadata in a payment envelope, gift-wraps it to the recipient (kind 1059, existing transport). Recipient's client validates and auto-reissues on read, then sends back an encrypted receipt. Unclaimed payments auto-refund via the cancel timeout ([ecash-over-nostr.md](ecash-over-nostr.md#oobnotes-mechanics)).

**Federation discovery**: the community announces its federation invite code via a NIP-87-compatible kind 38173 event (or community metadata). Clients auto-join (or prompt) on first wallet use. Notes can also embed the invite code as a fallback.

## Event kinds

Proposal: claim the free **50000-50999 block** ([buzz.md](buzz.md#kind-registry)). Sketch, numbers not final:

| Kind | Purpose | Class |
|---|---|---|
| 50001 | Payment envelope (inner event carried in a gift wrap: OOBNotes, amount, memo, federation id) | Never hits the relay unwrapped |
| 50002 | Payment receipt (claimed/refunded), possibly also gift-wrapped | Regular or wrapped |
| 50003 | Public payment marker for channel tips: amount + recipient visible, notes still encrypted to recipient | Regular, channel-scoped |
| 38173 | Federation announcement, NIP-87 interop | Parameterized replaceable (in-range, fits Buzz's rules) |

The DM path needs no relay changes at all (everything rides kind 1059). New kinds only become necessary for receipts-as-events, channel tips, and discovery, and each must be registered in `required_scope_for_kind()` or the relay rejects it ([buzz.md](buzz.md#feature-addition-pipeline)).

## Build phases

0. **Dependency spike.** Add fedimint 0.11.1 crates to the root workspace and to `desktop/src-tauri`, confirm both resolve. Known hazard: `secp256k1` skew (nostr 0.44 pins 0.29/0.31, fedimint uses the bitcoin 0.32 stack) plus `deny.toml` duplicate checks. Stand up the dev loop: `devimint` / `just mprocs` regtest federation.
1. **CLI wallet.** `buzz wallet join/balance/spend/reissue` against the regtest federation. Proves the embedding with no UI.
2. **Desktop wallet.** Tauri commands, keychain seed, send/receive inside DMs, minimal UI.
3. **Protocol formalization.** Payment envelope schema, receipt + tip kinds, federation discovery event, relay registration, e2e tests in `buzz-test-client`. Also sketches the agent spend-policy authorization (NIP-OA extension or a new grant kind), design only.
4. **Mobile.** flutter_rust_bridge infra + wallet screens. The biggest single lift ([buzz.md](buzz.md#clients)).
5. **Ambitious layer.** Agent wallets (implementing the Phase 3 spend-policy design), zap-like tipping UX, Lightning interop via gateway, Mutinynet staging federation.

## Phase 0 findings

Spike passed 2026-07-29. `crates/buzz-ecash` (fedimint-client + mint + ln + lnv2 + bip39 + rocksdb, all `0.11.1`) compiles, tests, and passes clippy/fmt/cargo-deny in the root workspace, and resolves + compiles under `desktop/src-tauri`.

- **secp256k1 skew: non-issue.** fedimint's bitcoin 0.32 stack unified onto the existing secp256k1 0.29.1. Duplicate crates after the spike: iroh x3 (0.35 fedimint, 0.90 fedimint "iroh-next", 1.0.2 buzz-relay-mesh), netwatch x3, socket2 x2, hickory-proto x2. All `warn`-level under `[bans]`.
- **netwatch 0.5.0 build break + workaround.** fedimint-connectors pins iroh 0.35 with default features off, so nothing enables socket2's `all` feature and netwatch 0.5.0 (which uses `all`-gated items without declaring the feature) fails to compile. Fix: buzz-ecash declares `socket2 = { version = "0.5", features = ["all"] }`; drop it when fedimint moves past iroh 0.35.
- **fedimint-wallet-client excluded.** On-chain peg-in/out is out of scope for chat e-cash, and the module drags in three RUSTSEC-flagged unmaintained proc-macro crates. Mint + lightning modules cover our flows.
- **Advisory triage in `deny.toml`.** Documented ignores added for: hickory-proto 0.25.2 DoS pair (via iroh 0.90, fix line unreachable), rustls-webpki 0.102.8 cert-validation quad (via iroh 0.35, only fedimint's own guardian connections use it, Buzz TLS is on the fixed 0.103 line), atomic-polyfill (target-gated embedded fallback, never compiled), proc-macro-error (compile-time doc macro via aquamarine). All tagged "remove when fedimint bumps iroh".
- **Dev loop verified.** A headless devimint regtest federation (4 guardians + bitcoind + LND/LDK gateways) boots from the local fedimint checkout, and `buzz-ecash` parses its real invite code to the exact federation id the guardians report. See the dev loop section below.

## Phase 1 notes

Complete 2026-07-29. `buzz-ecash` gained a real `Wallet` (`join`/`open`/`balance`/`info`/`spend`/`reissue` over fedimint-client 0.11) and `buzz-cli` gained `buzz wallet join|balance|spend|reissue|info` (data dir via `--data-dir` / `BUZZ_WALLET_DIR` / platform default; JSON out; federation errors exit 2). Verified end-to-end against a live devimint federation: join, fund with 100k msat from the devimint client, reissue, spend 20k back, balances reconcile exactly.

Implementation notes worth remembering:

- **Join flow** mirrors fedimint-cli: `Client::builder() -> preview(connectors, invite) -> join(db, RootSecret::StandardDoubleDerive(bip39))`. The mnemonic entropy is also stored inside the client db so standard fedimint tooling can open the wallet dir.
- **Mnemonic storage**: `mnemonic.txt` (0600, `create_new`) in the wallet dir is the source of truth for `open`. File storage is a Phase 1 compromise; desktop moves it to the keychain.
- **A failed join cleans the dir** (mnemonic, `client.db/`, sibling `client.db.lock`), so retries work instead of dying `AlreadyInitialized`.
- **Connectors**: `ConnectorRegistry::build_from_client_defaults()` initializes connectors lazily per URL scheme, so iroh never spins up for `ws://` federations.
- **Spend allows overpay** (`SelectNotesWithAtleastAmount`, fedimint-cli's behavior): exact-amount selection fails when denominations can't represent the amount. Callers must read `amount_msat` from the result.
- **Fees are federation-dependent, 0 or more.** Federations set up with older fedimintd versions charged no mint fees; newer ones (and the defaults going forward) do. Observed on a v0.11 devimint federation: 100,000 msat face -> 95,774 msat net after reissue. Never assume fees exist, never assume they don't: reissue returns note face value, balance returns what was actually credited, and payment UX (Phase 2+) should derive expected fees from the mint module's fee config or show measured deltas rather than hardcoding either case. The integration test asserts fee-agnostically (`0 < net <= face`-style bounds).
- **Reissue is awaited** via the operation update stream and rejects notes from a different federation by id prefix before submitting.
- Lightning modules (ln + lnv2) are registered so federation configs parse, but no lightning send/receive API is exposed yet. `Wallet` needs a multi-thread tokio runtime.

## Dev loop

The local fedimint checkout at `/Users/frank/Developer/fedimint` (tag `v0.11.1`, warm nix store and cargo build) is the dev federation host.

- **Interactive**: `cd /Users/frank/Developer/fedimint && nix develop -c just mprocs`. Boots the regtest federation plus a funded client behind an mprocs TUI (`ctrl+a q` quits, teardown included).
- **Headless / scripted** (inside `nix develop`):

  ```bash
  source scripts/_common.sh
  build_workspace && add_target_dir_to_path
  export FM_DEVIMINT_STATIC_DATA_DIR="$PWD/devimint/share"
  devimint --link-test-dir "${CARGO_BUILD_TARGET_DIR:-$PWD/target}/devimint" \
    dev-fed --exec <command>
  ```

  The exec'd command runs once the federation is ready, with `FM_INVITE_CODE` and `FM_CLIENT_DIR` set, and everything tears down when it exits.
- **Gotcha**: the dev shell sets `CARGO_BUILD_TARGET_DIR` to `target-nix`, so never hardcode `target/` in paths (devimint dies with a bare `No such file or directory` if the link-test-dir parent is missing).
- **Validating buzz-ecash against it**: `FM_INVITE_CODE=<code> FM_EXPECTED_FEDERATION_ID=<id> cargo test -p buzz-ecash -- --ignored` (the id comes from `fedimint-cli info | jq -r .federation_id`).

## Non-goals (for now)

- Trustless public tipping (needs P2PK-style locking Fedimint doesn't have).
- Wallet state synced through the relay (NIP-60 doesn't map to Fedimint).
- Relay-side or custodial wallets.
- Running a federation inside the relay process.
