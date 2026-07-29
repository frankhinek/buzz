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
