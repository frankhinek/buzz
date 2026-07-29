# Project: Fedimint E-Cash in Buzz

## Goal

Buzz users hold and send Bitcoin e-cash inside the app. Wallet lives on each device, payments travel as encrypted Nostr events, and a Buzz community maps to a Fedimint federation. See [integration-design.md](integration-design.md) for the architecture.

## Status

`2026-07-29`: research phase complete, no code written. Working branch `claude/buzz-fedimint-ecash-e8399d` (git worktree `buzz-fedimint-ecash-e8399d`).

Work happens on a public fork: `origin` = [frankhinek/buzz](https://github.com/frankhinek/buzz), `upstream` = [block/buzz](https://github.com/block/buzz) (Frank has read-only access upstream). Merge freely in the fork, sync `main` from upstream, PR to upstream only if this graduates.

Headline findings:

- Buzz has zero payments code today. Clean slate. Details in [buzz.md](buzz.md#existing-payments-surface).
- The P2P transport already exists: NIP-17 gift wraps carry encrypted payloads user-to-user with no relay changes. Details in [ecash-over-nostr.md](ecash-over-nostr.md).
- `fedimint-client` (Rust, v0.11.1) is directly embeddable and is how every serious Fedimint app is built. Details in [fedimint.md](fedimint.md#client-sdk-options).
- Desktop and CLI are native Rust hosts. Mobile is pure Dart with no FFI infra, the biggest single lift. Details in [buzz.md](buzz.md#clients).

## Open decisions

1. **Which client first.** CLI is the cheapest proof of embedding. Desktop is the visible one.
2. **Who runs federations.** A 4-guardian community federation is realistic in 2026, but guardians collectively custody member funds. Operational and regulatory weight, amplified by Block's name on the repo.
3. **Payment visibility.** Fully E2E-encrypted vs relay-visible receipts (better UX for confirmations and channel tips, leaks metadata).
4. **Agent wallets.** Technically trivial (an agent is a keypair), but "agent may spend up to N sats" needs a net-new authorization design. NIP-OA's condition grammar only expresses `kind=` and `created_at` bounds ([buzz.md](buzz.md#agent-surface)).
5. **Kind numbers.** Proposal: new 50000-50999 block ([integration-design.md](integration-design.md#event-kinds)).

## Risks

- Pre-1.0 API churn in fedimint crates, exact-version lockstep pinning, quarterly bump ritual.
- Heavy dependency tree compiled in two separate cargo workspaces (root + `desktop/src-tauri`).
- `secp256k1` version skew between nostr 0.44 and fedimint's bitcoin 0.32 stack. Phase 0 exists to de-risk this.
- No P2PK note locking in Fedimint: in-flight e-cash is claimable by whoever reads it, so encryption is mandatory and trustless public tipping (nutzap-style) is off the table for now.
- Recipients must join the sender's federation. Cross-federation means a Lightning gateway hop.
- Mobile FFI bridge is net-new build infrastructure.

## Next steps

Phase 0 dependency spike, then a CLI wallet. Full phasing in [integration-design.md](integration-design.md#build-phases).
