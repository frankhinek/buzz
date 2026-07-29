# Fedimint

Protocol fundamentals, client SDKs, release status, and what it takes to run a federation. Version and status claims verified against crates.io, npm, and GitHub 2026-07-29.

## Fundamentals

- A **federation** is `n = 3m+1` guardians each running `fedimintd` (4 tolerate 1 Byzantine fault). Guardians jointly hold a threshold-multisig Bitcoin wallet and threshold keys from a DKG ceremony. Solo 1-guardian mode exists for dev.
- Consensus is AlephBFT, ordering items into identical outcomes per session.
- **Module system**: server + client modules chosen at compile time, activated by federation config. Core: wallet (on-chain peg-in/out), mint (e-cash), ln (LNv1), lnv2, meta. Third-party modules are first-class (Fedi's stability pools, ROASTR). `mintv2`/`walletv2` are incubating in-tree.
- **E-cash**: Chaumian blind-signed bearer notes in power-of-2 msat denominations. A `SpendableNote` = note keypair + guardian threshold signature. Spend/reissue mechanics in [ecash-over-nostr.md](ecash-over-nostr.md#oobnotes-mechanics).
- **Lightning gateway**: an untrusted economic actor (not a guardian) swapping ecash for Lightning, one gateway can serve many federations. LNv2 (stabilized v0.5): client builds the contract with a chosen gateway, per-gateway fees, forfeit signatures on failed sends. Since v0.6 the gateway embeds LDK Node in a single `gatewayd` (CLN support dropped). BOLT12 in the gateway UI since v0.11.
- **Invite codes**: bech32 `fed1...` strings carrying guardian API endpoint(s) + the federation ID (hash identifier that authenticates the downloaded config).

## Client SDK options

| Option | Status (verified 2026-07-29) | Verdict |
|---|---|---|
| `fedimint-client` (Rust) | **0.11.1 stable** (2026-04-21), 0.12.0-beta.2 (2026-07-28), master 0.13.0-alpha | The real SDK. Directly embeddable. How Fedi, Harbor, Ecash App are built |
| `fedimint-clientd` / `multimint` / `fedimint-nwc` | Dormant, last commits Oct 2024 | Do not build on these |
| Fedimint Web SDK (`@fedimint/core` 0.1.3, wasm) | Active, alpha, APIs may change. `@fedimint/core-web` is a deprecated shim | Only if the web client needs a wallet |
| Flutter bindings | No official ones. `fedimint/ecash-app` (Flutter + flutter_rust_bridge 2.9, on Play Store, active) is the reference | Blueprint for Buzz mobile |
| `fedimint-sdk-ffi` (UniFFI) + `@fedimint/react-native` | Official, active, pre-release | Watch for mobile |

### Embedding `fedimint-client`

- Builder pattern: `Client::builder()` returns a `ClientHandle`. Client modules registered at compile time: `fedimint-mint-client` (primary, holds the balance), `fedimint-ln-client` / `fedimint-lnv2-client`, `fedimint-wallet-client`, `fedimint-meta-client`.
- Operations spawn background **state machines** driven by an executor. Calls return an `OperationId` you subscribe to for progress. The client is a stateful database, not a stateless signer.
- **Storage**: abstract `Database` trait. Official backends: `fedimint-rocksdb` (native) and `fedimint-cursed-redb` (wasm/OPFS). Fedi maintains its own redb backend, so the trait is genuinely third-party-implementable.
- **Keys**: `fedimint-bip39` derives the client root secret from a mnemonic.
- **Runtime**: tokio on native, compiles to wasm32. No declared MSRV, track recent stable.
- **Weight**: heavy. bitcoin 0.32, secp256k1, BLS12-381 threshold crypto, rocksdb, the full fedimint-core encoding stack.
- **Pinning**: all fedimint crates pin exact workspace versions (`=x.y.z`). The whole family upgrades in lockstep, roughly quarterly. Ecash App pins a git rev instead of crates.io releases.

### The Fedi bridge (architecture reference)

`fedibtc/fedi` (BSL Jan 2025, auto-converted AGPL-3.0 Jan 2026): one Rust core (`crates/bridge`) owns all wallet logic, exposed via UniFFI to React Native with a deliberately tiny surface (`fedimint_initialize` + string-in/string-out `fedimint_rpc` + an `EventSink` callback, types generated with ts-rs) and via wasm for a PWA. Runs on a fedimint fork with custom modules. Directly analogous to a "one Rust core, multiple shells" design for Buzz.

## Dev and test tooling

- `fedimint-cli`: in-repo wallet CLI for testing/debugging.
- `devimint` (crate + CLI): spins up a full local stack. `just mprocs` in the fedimint repo launches a 4-guardian regtest federation, bitcoind, and LND + LDK gateways.
- **Mutinynet** (30-second-block signet, faucet.mutinynet.com) is the standard shared test network, with public test federations.

## Running a federation (2026)

- Each guardian runs `fedimintd` + a bitcoin backend. Setup is a guided DKG ceremony in the web UI. Hardware demands are modest (Umbrel and Start9 packages exist, gateway memory under 200 MB since v0.11).
- v0.7's Iroh networking removed the public-IP/reverse-proxy requirement, home nodes work.
- A practical federation also needs someone operating a Lightning gateway.
- A 4-person team federation is realistic and is Fedimint's explicit target. Ongoing burden: uptime, lockstep guardian upgrades, and the custody reality that guardians collectively hold user funds.
- Discovery/monitoring: observer.fedimint.org (health + Nostr ratings), bitcoinmints.com (NIP-87 listings).

## Release status

Latest stable v0.11.1 "Mint Condition" (2026-04-21), v0.12 in beta, ~quarterly cadence. Highlights: v0.5 Tor + LNv2 stabilization, v0.6 LDK-in-gateway, v0.7 single-binary UI + LNURL + Iroh, v0.11 gateway mnemonic recovery + Pkarr guardian discovery + BOLT12. No 1.0, API churn between minors is real.

## Sources

- https://github.com/fedimint/fedimint (docs/architecture.md, docs/deploying.md, docs/tutorial.md, docs/lightning_module_v2.md)
- https://docs.fedimint.org/
- https://sdk.fedimint.org/ and https://github.com/fedimint/fedimint-sdk
- https://github.com/fedimint/fedimint-sdk-ffi
- https://github.com/fedimint/ecash-app
- https://github.com/fedibtc/fedi
- https://github.com/HarborWallet/harbor
- https://github.com/fedimint/awesome-fedimint
