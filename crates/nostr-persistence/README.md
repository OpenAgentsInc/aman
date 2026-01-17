# nostr-persistence

## Responsibility

Publishes and indexes Nostr events for document metadata, chunk references, and access policies. This crate provides a
publisher (write path) and an indexer (read path) that materializes events into SQLite.

Built on rust-nostr (`nostr-sdk`).

## Public interfaces

Consumes:

- DocManifest, ChunkRef, AccessPolicy
- relay URLs and signing keys

Produces:

- Nostr events (parameterized replaceable kinds)
- local SQLite tables: nostr_events, docs, chunks, policies

Traits:

- `NostrPublisher` (publish_doc_manifest, publish_chunk_ref, publish_policy)
- `NostrIndexer` (start, backfill, handle_event)

## Event kinds and tags

- DocManifest: kind 30090, tag d=doc_id
- ChunkRef: kind 30091, tag d=chunk_id
- AccessPolicy: kind 30092, tag d=scope_id
- Required tags: d, k, enc (if encrypted)

## How to run it

Publish a fixture:

```bash
cargo run -p nostr-persistence --bin nostr-publish-fixture -- \
  --relay wss://relay.damus.io \
  --key <NOSTR_SECRET_KEY> \
  --doc crates/nostr-persistence/fixtures/doc.json
```

Start the indexer:

```bash
cargo run -p nostr-persistence --bin nostr-indexer -- \
  --relay wss://relay.damus.io \
  --db ./data/nostr.db
```

## How to test it

- `cargo test -p nostr-persistence`
- Integration test (ignored by default):
  - set `NOSTR_TEST_RELAY` and `NOSTR_TEST_KEY`
  - run `cargo test -p nostr-persistence -- --ignored`

## Failure modes

- Relay retention policies drop custom kinds (NIP-11).
- Missing or mismatched `enc` tag prevents decrypting content.
- Quorum publish fails if too few relays ack the event.

## Security notes

- `NOSTR_SECRETBOX_KEY` enables symmetric encryption of payloads.
- Avoid publishing sensitive plaintext unless encryption is enabled.
