# Fedimint E-Cash for Buzz - Wiki

Durable knowledge base for adding Fedimint e-cash support to Buzz. This wiki is the project's memory: research learnings, design decisions, and work in progress live here so they survive context resets and session boundaries.

If you are an LLM session resuming this project: read this page, then [project.md](project.md) for current status, before re-deriving anything.

## Conventions

- Small topical pages, inter-linked with relative markdown links.
- Update pages in place as understanding improves. No parallel copies, no stale duplicates.
- Append a dated entry to [log.md](log.md) each working session.
- Volatile facts (crate versions, upstream repo status) carry a `verified YYYY-MM-DD` marker.
- External links go in a Sources section at the bottom of the page that uses them.
- Cite code as `path:line` so claims can be re-checked against the repo.

## Pages

| Page | One-liner |
|---|---|
| [project.md](project.md) | Goal, status, open decisions, risks, next steps |
| [buzz.md](buzz.md) | Buzz architecture map: feature pipeline, clients, key management, extension points |
| [fedimint.md](fedimint.md) | Fedimint protocol, e-cash mechanics, client SDKs, releases, running federations |
| [ecash-over-nostr.md](ecash-over-nostr.md) | Sending e-cash through Nostr events: transport, NIP precedents, trust model |
| [integration-design.md](integration-design.md) | Proposed Buzz+Fedimint architecture and build phases |
| [payment-protocol.md](payment-protocol.md) | Phase 3 event schemas: payment envelope, receipts, tips, federation discovery |
| [log.md](log.md) | Append-only session log |
