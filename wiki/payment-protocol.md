# Payment Protocol (Phase 3 draft)

Status: **draft for review, nothing implemented.** Event schemas for e-cash payments over Buzz's Nostr surface. Decisions this builds on: hybrid visibility, per-community federations, kinds in 50000-50999 ([project.md](project.md#decisions)). Background: [ecash-over-nostr.md](ecash-over-nostr.md) for OOBNotes semantics, [buzz.md](buzz.md#kind-registry) for the kind pipeline.

## Design constraints

- An OOBNotes string is bearer money: whoever reads it first can claim it. It must only ever exist inside NIP-44 ciphertext addressed to exactly one recipient.
- The relay must never see notes, amounts (for DM payments), or the wallet's existence beyond what the public tip marker deliberately reveals.
- Buzz DMs are relay-managed channels, not NIP-17 threads. Payments ride gift wraps for secrecy but carry an optional channel anchor so clients can place them in the DM timeline.
- Mint fees are 0 or more per federation: schemas carry face value only; credited amounts are reported in receipts, never assumed.

## Kinds

| Kind | Name | Class | Relay-visible? |
|---|---|---|---|
| 50001 | `KIND_ECASH_PAYMENT` | rumor only (inside kind 1059 gift wrap) | never (ingest rejects it bare) |
| 50002 | `KIND_ECASH_RECEIPT` | rumor only (inside gift wrap) | never (ingest rejects it bare) |
| 50003 | `KIND_ECASH_TIP_MARKER` | regular, channel-scoped (`h` tag) | yes, public within the channel |
| 38173 | `KIND_FEDERATION_ANNOUNCEMENT` | parameterized replaceable, NIP-87 compatible | yes |

50001/50002 are reserved in `kind.rs` for collision safety and documented as wrap-only; `required_scope_for_kind` keeps rejecting them at ingest, so the only relay work is 50003 + 38173 registration.

## 50001 - payment envelope (gift-wrapped rumor)

Standard NIP-59 nesting: gift wrap (1059, ephemeral key, p-gated to recipient) -> seal (13, signed by real sender) -> this unsigned rumor. Sender also self-wraps a copy so their other devices can reconstruct history.

```json
{
  "kind": 50001,
  "pubkey": "<sender>",
  "created_at": 1753800000,
  "tags": [
    ["p", "<recipient pubkey>"],
    ["h", "<dm channel uuid>"],
    ["e", "<tipped message id>", "", "tip"]
  ],
  "content": "{\"v\":1,\"federation_id\":\"<64-hex>\",\"amount_msat\":21000,\"notes\":\"<OOBNotes string>\",\"invite_code\":\"fed1...\",\"memo\":\"thanks!\",\"expires_at\":1753886400}"
}
```

- `h` is optional (anchors the payment into a DM channel timeline); the `e` tip tag is present only when the payment funds a channel tip (see 50003).
- `amount_msat` is the note face value. `invite_code` is optional (fallback for recipients not yet in the federation; usually redundant with 38173).
- `expires_at` mirrors the sender's `try_cancel_after` horizon: after this time the sender's wallet auto-reclaims unclaimed notes. Recipients should claim immediately; claims race cancellation near expiry.

Recipient flow: unwrap -> check `federation_id` against wallet (offer join via 38173/`invite_code` if absent) -> reissue immediately (auto-claim; deferring loses the race and leaves bearer data at rest) -> send a 50002 receipt. Double-processing the same envelope is safe: the second reissue fails at the federation, and the client should treat already-claimed as success if it was the claimer.

## 50002 - receipt (gift-wrapped rumor, recipient -> sender)

```json
{
  "kind": 50002,
  "tags": [["p", "<sender>"], ["e", "<50001 rumor id>"]],
  "content": "{\"v\":1,\"status\":\"claimed\",\"amount_msat\":21000}"
}
```

- `status`: `claimed` | `failed` | `declined`. `failed` carries an optional `reason` field (e.g. already spent, wrong federation).
- `amount_msat` is face value at claim; the recipient's credited amount may be lower on fee-charging federations and is nobody's business but theirs.
- Sender UX: pending until receipt or `expires_at` (then "returned to your wallet"). Receipts are best-effort; the money moved when reissue completed, not when the receipt arrived.

## 50003 - public tip marker (channel event)

The hybrid-visibility case: social proof in the channel, money still E2E. Two events per tip: the public marker in the channel, and a 50001 gift wrap (with the `e` tip tag) carrying the actual notes to the recipient.

```json
{
  "kind": 50003,
  "tags": [
    ["h", "<channel uuid>"],
    ["e", "<tipped message id>"],
    ["p", "<recipient>"]
  ],
  "content": "{\"v\":1,\"amount_msat\":21000,\"federation_id\":\"<64-hex>\"}"
}
```

- Relay registration: `MessagesWrite` scope, channel membership enforced like any channel event; triggers workflows (tip automations come free).
- The marker is a claim by the sender, not proof of payment. Clients may badge "claimed" only after the recipient's receipt (which stays private to the sender); render optimistically otherwise. Anyone can verify nothing here; that is inherent to keeping notes private and is acceptable for tips.

## 38173 - federation announcement (NIP-87 compatible)

Community admins publish which federation the community uses. Parameterized replaceable, keyed `(pubkey, 38173, d)`.

```json
{
  "kind": 38173,
  "tags": [
    ["d", "<federation id hex>"],
    ["u", "fed1<invite code>"],
    ["n", "regtest"],
    ["modules", "mint,ln,lnv2"]
  ],
  "content": ""
}
```

- Ingest accepts it only from relay members with admin/owner role (same pattern as other admin-gated kinds); ordinary members' announcements are rejected so the community federation cannot be spoofed.
- Clients: wallet join UX queries `{kinds:[38173]}`, trusts the newest admin-authored event. Interop bonus: standard NIP-87 clients (Amethyst, bitcoinmints) can read it.

## Agent spend policy (sketch only, per decision #4)

Enforcement cannot live at the relay (spends are wallet-local and federation-side). The enforcement point is the agent harness (`buzz-acp`), which owns the agent's wallet and refuses spends outside an owner-signed grant:

- New parameterized-replaceable kind (proposed 38180 `KIND_ECASH_SPEND_GRANT`), owner-authored, `d` = agent pubkey. Content: `{v, federation_id, max_msat_per_payment, max_msat_per_day, expires_at}`. Revocation = republish with zero limits.
- Agent-sent 50001 envelopes include a `grant` tag referencing the grant event so the owner (and recipients) can audit spends against the policy.
- NIP-OA's condition grammar stays untouched; the grant is a separate event, which avoids stretching a signature-conditions grammar into an accounting system.

Design only in Phase 3; implementation is Phase 5.

## Open questions for review

1. **Auto-claim on unwrap** (proposed: yes, immediately) vs claim-on-view. Auto-claim wins the cancel race and removes bearer data fastest, but means receiving a DM moves money without a user gesture.
2. **Group DMs are out of scope** for Phase 3 (notes are single-claimer; a group payment is first-reader-wins). Acceptable?
3. **Tip marker optimism**: marker renders before any proof of claim. Acceptable for tips, or should tips wait for a public-ish acknowledgment (which leaks recipient wallet activity)?
4. **Kind 38180 vs extending NIP-OA** for the spend grant.
5. **Mobile**: kind 1059 is not in the mobile kind mirror today, so mobile cannot see DM payments until the FRB wallet work (Phase 4) plus gift-wrap support land. Fine to ship desktop-first?
