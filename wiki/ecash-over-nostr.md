# E-Cash over Nostr

How Fedimint e-cash travels through a Nostr relay, what the NIP ecosystem already provides, and the trust model. Background: [buzz.md](buzz.md#encrypted-messaging-the-e-cash-transport) for the Buzz side, [fedimint.md](fedimint.md#fundamentals) for the Fedimint side.

## The transport already exists in Buzz

NIP-17 gift wraps (kind 1059) are stored, p-gated, search-excluded, and carry up to 256 KB of NIP-44 ciphertext. An OOBNotes string is small text. A gift-wrapped e-cash payment works against today's relay with zero changes. Gift wraps are WebSocket-only in Buzz (rejected on the HTTP bridge).

## OOBNotes mechanics

- `spend_notes()` (CLI: `fedimint-cli spend <msats>`) produces an **`OOBNotes`** string: base64 (or base32) consensus-encoded, containing a `FederationIdPrefix` + `TieredMulti<SpendableNote>`, optionally an embedded invite code (`new_with_invite`) so a non-member recipient can join first. The format is a list of typed parts, unknown parts ignored for forward compatibility.
- Recipient flow: `validate_notes` (checks federation prefix, signatures, spend keys, returns the amount), then `reissue_external_notes` swaps them for freshly blinded notes. **Reissue is the double-spend protection.** Until the recipient reissues, the sender still holds valid copies.
- Notes never expire cryptographically. `spend_notes(try_cancel_after, ...)` makes the sender's client auto-reclaim after a timeout, and `try_cancel_spend_notes` is the manual version. Cancellation **races** the recipient's reissue. For chat: send with a generous timeout, recipient claims on read, unclaimed funds auto-return.

## Trust model

- Bearer instrument. The string is the money.
- **No recipient locking.** Cashu has NUT-11 P2PK, Fedimint OOB notes have nothing equivalent. Whoever reads the string first can claim it. E2E encryption is the only protection, which rules out trustless public payments.
- **Federation-scoped.** The embedded federation-ID prefix ties notes to one federation. The recipient must be (or become) a member to reissue. Cross-federation transfer = a Lightning hop through each side's gateway, or on-chain.

## NIP precedents

| NIP | What it is | Applies to Fedimint? |
|---|---|---|
| NIP-60 | Cashu wallet state stored on relays (kinds 17375/7375/7376) | No. Works because Cashu proofs are self-contained bearer data against a stateless HTTP mint. A fedimint client is a stateful state-machine DB, "wallet on relays" doesn't map. Nobody has shipped a Fedimint equivalent |
| NIP-61 (nutzaps) | P2PK-locked Cashu tokens published as events, redeemable at leisure | No. Requires P2PK locking, which Fedimint lacks |
| NIP-87 | Ecash mint discoverability. **Kind 38173 = Fedimint federation announcements** (invite codes in `u` tags), 38000 = user recommendations | Yes. Adopted by Amethyst, Ecash App, Vipr, bitcoinmints.com. The established Fedimint-Nostr touchpoint, and the natural way to announce a community's federation |
| NIP-57 (zaps) | LN-invoice-based tips | Indirectly. Fedimint wallets participate through the Lightning gateway |
| NIP-47 (NWC) | Wallet remote control | `fedimint-nwc` is dormant. Living implementations: Ecash App ships built-in NWC, Vipr does Lightning via NWC |

## Prior art for e-cash in chat

- **Fedi** embeds fedimint payments in Matrix rooms. Same UX pattern, different transport. The closest production system to what Buzz would build.
- **Mutiny Wallet** shipped fedimint support Dec 2023, shut down Dec 2024. Its successor **Harbor** (Rust/iced desktop, Fedimint + Cashu multi-mint) is alive and nearing 1.0.
- Pasting OOB notes into Nostr DMs is done informally today.
- Curiosity: the **ROASTR** module has guardians threshold-sign Nostr events.

## Sources

- https://github.com/nostr-protocol/nips/blob/master/60.md
- https://nips.nostr.com/61
- https://github.com/nostr-protocol/nips/blob/master/87.md
- https://docs.fedimint.org/fedimint_mint_client/struct.OOBNotes.html
- https://github.com/ngutech21/vipr-wallet
