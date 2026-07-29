# Log

Append-only. Newest entry at the bottom. Prefix entries with `YYYY-MM-DD`.

## 2026-07-29 - Research phase, wiki created

- Mapped the Buzz codebase: feature-addition pipeline, kind registry, DM/gift-wrap transport, client hosts, key management. Findings in [buzz.md](buzz.md).
- Researched Fedimint fundamentals and the 2026 SDK landscape. Findings in [fedimint.md](fedimint.md).
- Assessed e-cash-over-Nostr precedents (NIP-60/61/87) and OOBNotes semantics. Findings in [ecash-over-nostr.md](ecash-over-nostr.md).
- Drafted the recommended architecture (shared `buzz-ecash` crate, wallets on devices, community-to-federation mapping) and build phases in [integration-design.md](integration-design.md).
- No code written yet. Open decisions in [project.md](project.md).
- Created public fork `frankhinek/buzz` and rewired remotes (`origin` = fork, `upstream` = block/buzz, which is read-only for us). Pushed this branch to the fork.

## 2026-07-29 - Direction decided

- CLI-first build order, per-community federation config, hybrid payment visibility (E2E DMs, optional public tip markers), agent spend-policy designed in Phase 3 and built in Phase 5, kinds in the 50000-50999 block. Record in [project.md](project.md#decisions).
- Next up: Phase 0 dependency spike per [integration-design.md](integration-design.md#build-phases).

## 2026-07-29 - Phase 0 dependency spike

- Added `crates/buzz-ecash` with the fedimint 0.11.1 client stack (mint + ln + lnv2, no on-chain wallet module). Compiles + tests + clippy/fmt/deny green in the root workspace; resolves and compiles under `desktop/src-tauri` too.
- secp256k1 skew turned out to be a non-issue. The real friction was iroh: fedimint exact-pins iroh 0.35 + 0.90, breaking netwatch 0.5.0 (socket2 `all`-feature workaround applied) and importing RUSTSEC baggage (documented ignores in `deny.toml`).
- Findings recorded in [integration-design.md](integration-design.md#phase-0-findings). Remaining Phase 0 item: stand up the devimint regtest federation (prebuilt Apple Silicon binaries exist).

## 2026-07-29 - Phase 0 complete: devimint dev loop verified

- Booted a headless devimint regtest federation (4 guardians, bitcoind, gateways) from the local fedimint checkout at `/Users/frank/Developer/fedimint` (v0.11.1, warm nix + cargo caches made it fast).
- `buzz-ecash` parsed the live invite code and derived the same federation id the federation reports (`fedimint-cli info`). Repeatable via the new ignored test and the [dev loop](integration-design.md#dev-loop) recipe.
- Phase 1 (CLI wallet: join/balance/spend/reissue) is unblocked.
