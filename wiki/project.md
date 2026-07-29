# Project: Fedimint E-Cash in Buzz

## Goal

Buzz users hold and send Bitcoin e-cash inside the app. Wallet lives on each device, payments travel as encrypted Nostr events, and a Buzz community maps to a Fedimint federation. See [integration-design.md](integration-design.md) for the architecture.

## Status

`2026-07-29`: research phase complete, no code written. Working branch `claude/buzz-fedimint-ecash-e8399d` (git worktree `buzz-fedimint-ecash-e8399d`).

Work happens on a public fork: `origin` = [frankhinek/buzz](https://github.com/frankhinek/buzz), `upstream` = [block/buzz](https://github.com/block/buzz) (Frank has read-only access upstream). Merge freely in the fork, sync `main` from upstream, PR to upstream only if this graduates.

`2026-07-29`: **Phase 0 dependency spike passed.** `crates/buzz-ecash` compiles with the fedimint 0.11.1 stack in both the root workspace and `desktop/src-tauri`. Findings in [integration-design.md](integration-design.md#phase-0-findings). Next: the devimint regtest loop, then the Phase 1 CLI wallet.

Headline findings:

- Buzz has zero payments code today. Clean slate. Details in [buzz.md](buzz.md#existing-payments-surface).
- The P2P transport already exists: NIP-17 gift wraps carry encrypted payloads user-to-user with no relay changes. Details in [ecash-over-nostr.md](ecash-over-nostr.md).
- `fedimint-client` (Rust, v0.11.1) is directly embeddable and is how every serious Fedimint app is built. Details in [fedimint.md](fedimint.md#client-sdk-options).
- Desktop and CLI are native Rust hosts. Mobile is pure Dart with no FFI infra, the biggest single lift. Details in [buzz.md](buzz.md#clients).

## Decisions

Decided 2026-07-29:

1. **CLI first.** `buzz wallet` subcommands prove the embedding with zero UI work. Desktop follows on the proven crate.
2. **Per-community federation.** Community admins configure one federation, announced via a NIP-87-style event, and members' wallets auto-join it. Dev and staging run on devimint regtest and Mutinynet. Who custodies real funds is deferred until launch readiness.
3. **Hybrid payment visibility.** DM payments and receipts are fully E2E-encrypted (relay sees only gift wraps). Channel tips may emit a public marker event (amount, sender, recipient) while the notes stay encrypted to the recipient.
4. **Agent wallets: design in Phase 3, build later.** The Phase 3 protocol work sketches the spend-policy authorization (NIP-OA extension or a new grant kind, see [buzz.md](buzz.md#agent-surface)) so schemas need no breaking changes when agents arrive. Implementation stays in Phase 5.
5. **Kind numbers: the 50000-50999 block** as sketched in [integration-design.md](integration-design.md#event-kinds).

## Risks

- Pre-1.0 API churn in fedimint crates, exact-version lockstep pinning, quarterly bump ritual. Phase 0 confirmed a second cost: fedimint's exact iroh pins (0.35 + 0.90) drag in old duplicates carrying RUSTSEC advisories, so each fedimint bump means re-triaging `deny.toml` ignores.
- Heavy dependency tree compiled in two separate cargo workspaces (root + `desktop/src-tauri`). Confirmed workable in Phase 0.
- ~~`secp256k1` version skew~~ resolved: fedimint's bitcoin 0.32 stack unified onto the workspace's existing secp256k1 0.29.1, no new duplicate.
- No P2PK note locking in Fedimint: in-flight e-cash is claimable by whoever reads it, so encryption is mandatory and trustless public tipping (nutzap-style) is off the table for now.
- Recipients must join the sender's federation. Cross-federation means a Lightning gateway hop.
- Mobile FFI bridge is net-new build infrastructure.

## Next steps

Direction is decided, Phase 0 (dependency spike) is unblocked, then the CLI wallet. Full phasing in [integration-design.md](integration-design.md#build-phases).
