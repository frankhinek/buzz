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
  "content": "{\"v\":1,\"federation_id\":\"<64-hex>\",\"amount_msat\":21000,\"notes\":[\"<OOBNotes string>\"],\"invite_code\":\"fed1...\",\"memo\":\"thanks!\",\"expires_at\":1753886400}"
}
```

- `h` is optional (anchors the payment into a DM channel timeline); the `e` tip tag is present only when the payment funds a channel tip (see 50003).
- `notes` is an **array** of OOBNotes strings, all from `federation_id`. Phase 3 senders emit exactly one; the array exists so the future spend-anywhere float can compose exact amounts from denominated bundles (see multi-device model). Recipients reissue every entry; the receipt reports the total actually claimed.
- `amount_msat` is the total note face value. `invite_code` is optional (fallback for recipients not yet in the federation; usually redundant with 38173).
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

## Multi-device model

One nsec, many devices: NIP-AB pairing copies the identity, so every device decrypts payment envelopes. Fedimint clients cannot share a root secret (a client is a live state machine over a local db; recovery is a rescan, not sync), so wallets are inherently per-device. The model, settled 2026-07-30:

- **Exactly one treasury.** The device that joined the federation is the wallet device; the user's other devices show a pointer ("your wallet is on iPhone") plus the treasury's balance, never a wallet of their own. Recommended default treasury once mobile exists: the phone (most online, most at hand). Multi-wallet and aggregate-balance UX were rejected as permanently confusing (balance must have one answer).
- **Auto-claim, treasury only.** The treasury reissues incoming envelopes immediately on unwrap. Rationale: mainstream P2P apps (Venmo, Cash App) auto-credit, so the consent concern is settled by precedent; immediate claim destroys bearer data fastest and wins the sender-cancel race. Scoping claims to the treasury removes self-racing entirely. Flood hardening (client-side rate limit on reissue attempts per sender) is deferred until it matters; no protocol impact.
- **38181 `KIND_ECASH_WALLET_DESIGNATION`** (parameterized replaceable, author-only, content NIP-44 to self): `d` = federation id, content `{v, device_id, device_label, balance_msat, updated_at}`. Gives every device the same balance number and names the treasury. Device ids are generated at wallet creation; duplicate ids across snapshots indicate a cloned machine (a fedimint hazard worth warning about). Moving the treasury is a deliberate later flow (sweep via a self-addressed envelope).
- **Future: spend-anywhere float (Phase 6, re-evaluate first).** Designed, not implemented: the treasury pre-spends small denominated OOBNotes bundles with long expiries and publishes them encrypted to self; any device composes exact amounts from bundles (why `notes` is an array), marks them used via an LWW self-event, and sends without the treasury or the federation in the loop. The payee reissues. Exposure is bounded to the float (nsec compromise cannot reach the treasury), semantics are NWC-like (a spending budget that does not return to the root wallet), and an offline sender can even hand envelopes over QR. Costs to weigh at re-evaluation: top-up fees on fee-charging federations (each denomination bundle is its own spend), bundle-expiry bookkeeping (a payment's expiry must fit inside its bundles' remaining lifetime), race-then-retry when two devices grab the same bundle. Kind 38182 `KIND_ECASH_FLOAT_BUNDLE` is reserved.
- **Parked: relay note pool** (all notes published encrypted to self, any device spends any note). Rejected for now: every balance change becomes a fee-bearing rotation, concurrent pool updates conflict, and nsec compromise alone would expose the entire balance. Revisit alongside P2PK-style recipient locking if fedimint ever grows it.

## Review status

Approved by Frank 2026-07-29:

- **Group DMs are out of scope** for Phase 3 (notes are single-claimer; a group payment is first-reader-wins). 1:1 DM payments only.
- **Desktop-first shipping.** Kind 1059 is not in the mobile kind mirror today, so mobile cannot see DM payments until the FRB wallet work (Phase 4) plus gift-wrap support land.

Approved by Frank 2026-07-30:

- **Auto-claim on unwrap**, scoped to the treasury device (see multi-device model).
- **Single-treasury multi-device model**; spend-anywhere float deferred to Phase 6; relay note pool parked.

Still open:

1. **Tip marker optimism**: marker renders before any proof of claim. Acceptable for tips, or should tips wait for a public-ish acknowledgment (which leaks recipient wallet activity)?
2. **Kind 38180 vs extending NIP-OA** for the spend grant.
