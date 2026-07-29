# Buzz Architecture Map

What matters about Buzz for the e-cash project. Full picture in the repo's `ARCHITECTURE.md` and `CLAUDE.md`. Facts verified against the codebase 2026-07-29.

Buzz is Nostr-first: every feature is a signed event with a `kind` integer, and the relay (Axum + Postgres + Redis) is the single source of truth. New feature = new kind, zero breaking changes for existing clients.

## Kind registry

`crates/buzz-core/src/kind.rs` is authoritative. All kinds are `u32` constants, but they must fit in the nostr crate's u16-backed `Kind` (max 65535, compile-time asserted).

Persistence class is derived purely from the number range:

| Range | Class |
|---|---|
| 0, 3, 41, 10000-19999 | Replaceable, keyed `(pubkey, kind)` |
| 20000-29999 | Ephemeral, never stored, Redis pub/sub only |
| 30000-39999 | Parameterized replaceable (NIP-33), keyed `(pubkey, kind, d_tag)` |
| everything else | Regular append-only stored events |

Access-control classes are explicit constant slices in `kind.rs`:

- `AUTHOR_ONLY_KINDS` (`kind.rs:120`): readable only by the author.
- `P_GATED_KINDS` (`kind.rs:146`): readable only by pubkeys in the event's `#p` tag. Includes `KIND_GIFT_WRAP` (1059). Persistent p-gated kinds get NULL `search_tsv` so FTS never sees them.
- `RESULT_GATED_KINDS` (`kind.rs:129`): gate holds even for `{ids:[...]}` lookups.
- `is_relay_only_kind` (`kind.rs:758`): client submission rejected.
- `is_command_kind` (`kind.rs:743`): transactional execution.

Claimed custom ranges: 42000 feedback, 43000-43006 agent jobs, 44100-44200 notifications/metrics, 45000-45003 forum, 46000-46031 workflow, 47000-47999 reserved but empty, 48000-48106 system/huddle, 49001 media. Convention is a fresh 1000-block per feature. **50000-50999 is free** and is the proposed e-cash block ([integration-design.md](integration-design.md#event-kinds)).

## Feature-addition pipeline

Documented in `CONTRIBUTING.md:411-477`. A new feature touches, in order:

1. Kind constant in `crates/buzz-core/src/kind.rs` + `ALL_KINDS` (duplicate-detection test at `kind.rs:828`, range assertions via `const _` blocks).
2. Payload struct in `buzz-core`.
3. Auth scope in `required_scope_for_kind()`, `crates/buzz-relay/src/handlers/ingest.rs:211`. **Unknown kinds are hard-rejected** (`ingest.rs:316`), so the relay must know the kind before clients can use it.
4. Post-storage side effects in `handle_side_effects()`, `crates/buzz-relay/src/handlers/side_effects.rs:194`. Optional HTTP surface in `crates/buzz-relay/src/api/` + `router.rs`.
5. Persistence module in `crates/buzz-db/src/` + migration `migrations/NNNN_snake.sql` (currently 0001-0025, next is 0026). Schema mirror at `schema/schema.sql` is test-guarded.
6. Event builders in `crates/buzz-sdk/src/builders.rs`.
7. CLI subcommand in `crates/buzz-cli/src/lib.rs` + `crates/buzz-cli/src/commands/`.
8. Kind mirrors, kept in sync by hand: `desktop/src/shared/constants/kinds.ts` and `mobile/lib/shared/relay/nostr_models.dart`.
9. Tests: serde in `buzz-core`, e2e in `crates/buzz-test-client/tests/`.

Reference trace: reactions (kind 7) and canvas (40100) follow exactly this path, builders at `builders.rs:463-529`.

## Existing payments surface

None. Exhaustive grep for zap/ecash/cashu/fedimint/lightning/bolt11/invoice/nwc/lnurl across Rust, Dart, TS, and both lockfiles found only `payment_required: false` in `crates/buzz-relay/src/nip11.rs:110`. The `nostr` crate (workspace v0.44, resolves 0.44.6) is compiled with only `nip44` + `nip98` features. No wallet crates anywhere. No stubs or TODOs.

## Encrypted messaging (the e-cash transport)

- DMs use NIP-17 gift wraps: `KIND_GIFT_WRAP` = 1059, stored, p-gated, search-excluded.
- Content is opaque NIP-44 ciphertext up to `MAX_EVENT_CONTENT_BYTES` = 256 KB (`ingest.rs:1528`).
- Ingest exempts gift wraps from the pubkey-must-match-auth check because NIP-59 uses an ephemeral pubkey (`ingest.rs:1537`).
- Gift wraps are WebSocket-only, rejected on the HTTP bridge (`ingest.rs:1488`).
- Crypto is the nostr crate's NIP-44 v2, no bespoke primitives. Usage pattern in `crates/buzz-core/src/engram.rs:133`.
- DM conversation persistence in `crates/buzz-db/src/dm.rs`.
- `validate_engram_nip44_content()` (`ingest.rs:1108`) is a reusable template for shape-checking NIP-44 ciphertext without keys.

Net: an e-cash note can ride a gift wrap today with zero relay changes.

## Clients

| Client | Stack | Key storage |
|---|---|---|
| Desktop | Tauri 2 + React 19. Rust backend with 266 `#[tauri::command]` handlers, native tokio WS to the relay (`native_websocket.rs`), bundled SQLite (`retention.db`) | OS keychain via `keyring` 3.6.3, `desktop/src-tauri/src/secret_store.rs` |
| Mobile | Flutter, **pure Dart**. No flutter_rust_bridge, no dart:ffi. Dart `nostr: ^2.0.0`. Kind mirror covers a subset (currently no 1059) | `flutter_secure_storage: ^10.0.0` |
| CLI | Rust (`buzz-cli`) | `BUZZ_PRIVATE_KEY` env var, no keychain |
| Agents | `buzz-acp` harness spawns subprocesses | `BUZZ_PRIVATE_KEY` env-injected per child |

`desktop/src-tauri` is **excluded from the root cargo workspace** and has its own lockfile. Any shared wallet crate must compile under both dependency resolutions.

Workspace crypto deps to reconcile with fedimint: `secp256k1` 0.29.1 and 0.31.1 both present (transitive via nostr), `bitcoin_hashes` 0.14.1. Fedimint pins the bitcoin 0.32 stack. There is a `deny.toml` at the root that may flag duplicates.

## Agent surface

Agents are keypairs authorized via NIP-OA owner attestation (`crates/buzz-sdk/src/nip_oa.rs`). The condition grammar (`nip_oa.rs:36-107`) only expresses `kind=N`, `created_at<N`, `created_at>N`. There is no spend-limit or payment-scope primitive. Agent spending policy is net-new design, scheduled for Phase 3 ([project.md](project.md#decisions)).

## Relay extension points

Patterns for relay-side services, in case a community-level federation service is ever needed:

- `buzz-workflow`: post-store hook `engine.on_event()` + outbound webhooks via `ActionSink`.
- `buzz-media`: Blossom/S3 blob storage with its own auth, HTTP surface under `crates/buzz-relay/src/api/media.rs`.
- Git hosting: full smart-HTTP server embedded in the relay, driven by NIP-34 events.
- `buzz-relay-mesh`: inter-relay QUIC mesh over iroh.
