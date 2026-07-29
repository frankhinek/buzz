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

## Non-goals (for now)

- Trustless public tipping (needs P2PK-style locking Fedimint doesn't have).
- Wallet state synced through the relay (NIP-60 doesn't map to Fedimint).
- Relay-side or custodial wallets.
- Running a federation inside the relay process.
