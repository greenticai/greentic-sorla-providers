# FoundationDB Provider

PR-04 introduces the first real SoRLa provider implementation: the FoundationDB provider.

## Current Scope

This first cut is local and development oriented, but it implements real provider behavior rather than metadata-only stubs:

- append immutable events with expected-revision checks
- read event streams by stream ID and revision
- persist projection snapshots
- read projection snapshots
- emit projection checkpoints
- rebuild projections from a checkpoint token or the full stream horizon
- persist canonical SORX entities and apply atomic canonical writes in the local/dev backend

The backing store in this repo is an in-memory transactional model that mirrors the keyspace layout and behavior we want for the later external FoundationDB runtime path. That keeps the semantics testable now while avoiding a hard dependency on a running FoundationDB cluster in CI.

## Keyspace Conventions

The provider reserves these logical key prefixes under the configured tenant prefix:

- `events`
- `projections`
- `indexes`
- `metadata`
- `checkpoints`
- `compatibility`

Example with tenant prefix `tenant/acme`:

- `tenant/acme/events`
- `tenant/acme/projections`
- `tenant/acme/indexes`
- `tenant/acme/metadata`
- `tenant/acme/checkpoints`
- `tenant/acme/compatibility`

## Canonical SORX Keyspace

Canonical source-of-record paths are encoded under `/sorx/{tenant}/{sor}`. The provider percent-encodes key segments deterministically: ASCII letters, digits, `-`, `_`, and `.` are preserved, while all other bytes are encoded as uppercase `%XX`. For example, tenant `tenant/acme` becomes `tenant%2Facme`.

The canonical keyspace intentionally uses `tenant_id + sor_id` only. `environment_id`, deployment IDs, and generated bundle or pack versions must not alter the production source-of-record prefix.

Current canonical prefixes:

- `/sorx/{tenant}/{sor}/meta/current_schema`
- `/sorx/{tenant}/{sor}/meta/canonical_version`
- `/sorx/{tenant}/{sor}/meta/active_views`
- `/sorx/{tenant}/{sor}/entities`
- `/sorx/{tenant}/{sor}/entity_versions`
- `/sorx/{tenant}/{sor}/events`
- `/sorx/{tenant}/{sor}/events_by_entity`
- `/sorx/{tenant}/{sor}/events_by_time`
- `/sorx/{tenant}/{sor}/edges/out`
- `/sorx/{tenant}/{sor}/edges/in`
- `/sorx/{tenant}/{sor}/indexes`
- `/sorx/{tenant}/{sor}/composite_indexes`
- `/sorx/{tenant}/{sor}/external_refs`
- `/sorx/{tenant}/{sor}/external_refs_by_entity`
- `/sorx/{tenant}/{sor}/evidence`
- `/sorx/{tenant}/{sor}/evidence_by_entity`
- `/sorx/{tenant}/{sor}/migrations`
- `/sorx/{tenant}/{sor}/deployments`

## Canonical Writes

`CanonicalWriteProvider::apply_canonical_write` applies one canonical event, canonical entity projection, relationship updates, and evidence/entity links under one provider state lock in the local/dev backend. Duplicate idempotency keys are rejected before any state mutation.

The current implementation still does not connect to a live FoundationDB runtime; the in-memory backend preserves the contract shape and deterministic behavior for tests.

## Config Shape

Current config schema fields:

- `cluster_file`
- `tenant_prefix`

`cluster_file` is kept in the schema now because the intended production runtime remains FoundationDB-backed even though the current repo implementation uses a local transactional model for CI and development safety.

## Durability & Recovery

### Feature gate

The real FoundationDB backend is compiled in only when the `foundationdb-real` Cargo feature is enabled:

```toml
provider-foundationdb = { features = ["foundationdb-real"] }
```

Without that feature the crate builds cleanly without any FoundationDB client library dependency. The `Backend` enum inside `FoundationDbProvider` has two variants — `Memory` (default) and `Fdb` (gated). `FoundationDbProvider::for_tests()` always uses `Memory` regardless of the feature flag.

### Cluster wiring

`FoundationDbProvider::new` connects to a real cluster when the `foundationdb-real` feature is active. Cluster discovery works in priority order:

1. If `cluster_file` in the config is non-empty, it is written to the `FDB_CLUSTER_FILE` environment variable before opening the database.
2. If `cluster_file` is empty (or the field is omitted), `connect()` inherits whatever `FDB_CLUSTER_FILE` is already set in the process environment.

Two public functions manage the FDB network lifecycle:

- `boot_network()` — starts the FDB client network thread and returns a `NetworkAutoStop` guard. The guard **must** be held for the lifetime of all DB use; dropping it shuts the network thread down cleanly. `boot_network()` must be called exactly once per process, before any `Database` handle is opened. Callers own this responsibility (process startup or the top of a test file).
- `connect(cluster_file: Option<&str>)` — opens a `Database` and builds a current-thread Tokio runtime without booting the network. The caller must already hold a live `NetworkAutoStop` guard before calling `connect()`.

If `connect()` fails (e.g. cluster unreachable), `FoundationDbProvider::new` falls back silently to the in-memory backend so metadata and config-validate calls still work; the degraded state is observable through `health()`.

### Atomic canonical write

`apply_canonical_write` on the `Fdb` backend executes a single `Database::run` transaction that writes all of the following atomically, or commits nothing:

1. **Idempotency marker** — if `idempotency_key` is set, its presence is checked first inside the transaction. A duplicate key causes immediate rejection (`ProviderError::Validation`) and nothing is persisted.
2. **Optimistic sequence guard** — the current stream head is read inside the transaction. If `event.sequence != head + 1`, the write is rejected. This prevents gaps and out-of-order events.
3. **Immutable event** — the `SorEventRecord` is CBOR-encoded and written under its sequence key.
4. **Canonical entity projection** — the `CanonicalEntityRecord` is upserted under the entity key.
5. **Relationship edges** — each relationship is written under both an outgoing edge key and an incoming edge index key.
6. **Entity links** — each `EntityLink` is written keyed by entity token, source reference, and match kind.

Because `Database::run` may retry the closure on FDB conflicts, all inputs are cloned into the closure so re-execution is safe. Domain errors (duplicate idempotency, sequence conflict, codec failure) surface as `ProviderError::Validation`.

### Restart survival

Data written through the `Fdb` backend lives in the external FoundationDB cluster and survives the `FoundationDbProvider` being dropped and recreated. The `fdb_durability_survives_recreation_and_rebuild` test in `tests/fdb_durability.rs` (gated on `foundationdb-real`) verifies this:

- A canonical entity written by one provider instance is readable by a fresh instance constructed from the same config.
- A projection checkpoint persisted by one instance is recoverable via `rebuild_projection` on a fresh instance.

### Projection checkpoints

`persist_projection` on the `Fdb` backend writes the `ProjectionRecord` and the `last_applied_revision` atomically in one transaction. `rebuild_projection` reads the stored `last_applied_revision` from the checkpoint key (returning 0 when absent) and returns a `ProjectionCheckpoint` whose token is `{projection_name}@{revision}`. This token survives provider recreation because it is backed by the cluster.

The projection namespace is derived deterministically from `config.tenant_prefix` with a fixed `sor_id` of `"projections"`, ensuring that a record persisted by one provider instance is addressable by any other instance with the same config.

### Keyspace layout on the FDB backend

All real-cluster keys are built by `FdbKeyspace` under the root `/sorx/{tenant}/{sor}`, where both segments are percent-encoded (ASCII letters, digits, `-`, `_`, `.` pass through; all other bytes become uppercase `%XX`). `environment_id` is never included in any key prefix so production source-of-record keys are stable across environments.

Key subspaces on the real backend:

| Subspace | Pattern |
|---|---|
| Events | `/sorx/{tenant}/{sor}/events/{stream_enc}/{seq:020}` |
| Event stream head | `/sorx/{tenant}/{sor}/events/{stream_enc}/_head` |
| Canonical entities | `/sorx/{tenant}/{sor}/entities/{type}\u{1f}{id}` |
| Outgoing edges | `/sorx/{tenant}/{sor}/edges/out/{rel_type}\u{1f}{from}\u{1f}{to}` |
| Incoming edges | `/sorx/{tenant}/{sor}/edges/in/{rel_type}\u{1f}{to}\u{1f}{from}` |
| Idempotency markers | `/sorx/{tenant}/{sor}/idempotency/{key}` |
| Entity links | `/sorx/{tenant}/{sor}/links/{entity_token}\u{1f}{source_ref}\u{1f}{match_kind}` |
| Projection records | `/sorx/{tenant}/{sor}/projections/{name}\u{1f}{key}` |
| Projection checkpoints | `/sorx/{tenant}/{sor}/checkpoints/{name}` |

Event sequence keys are zero-padded to 20 digits so byte order matches numeric order, enabling efficient range scans. The `_head` sentinel key starts with `_` (0x5f), which sorts after all digit-prefixed sequence keys, and is excluded from event range scans by using `:` (0x3a) as the exclusive upper bound.

Values are encoded with CBOR (via `ciborium`), matching the IR canonical codec.

The `\u{1f}` character (ASCII unit separator) is used as the field delimiter inside composite key suffixes.

### Methods not yet supported on the FDB backend

The following provider methods return `ProviderError::Validation("... not yet supported on the FoundationDB backend")` when called against the `Fdb` variant. They work normally against the in-memory backend:

- `append_event` / `read_event_stream` (legacy event store)
- `upsert_entity` / `get_entity` / `search_entities` (generic entity store)
- `upsert_canonical_entity` (standalone canonical entity upsert; use `apply_canonical_write` instead)
- `upsert_relationship` / `upsert_evidence_link` / `evidence_links_for_entity`
- `query_relationships` / `find_paths` (ontology graph traversal)
- `link_entities` (entity linking query)
- `projection_checkpoint` (direct checkpoint accessor)
- `query_metric` / `metric_rows_for_query` (metrics)

### CI

The `fdb-tests` job in `.github/workflows/ci.yml` runs the gated tests against a real cluster:

1. Installs the FoundationDB 7.3.27 client library.
2. Starts a `foundationdb/foundationdb:7.3.27` Docker container in single-node memory mode.
3. Writes the cluster file from the container to `$RUNNER_TEMP/fdb.cluster`.
4. Runs `cargo test -p provider-foundationdb --features foundationdb-real` with `FDB_CLUSTER_FILE` pointing at that file.

The default CI job (`provider-check`) runs without the `foundationdb-real` feature and requires no cluster. The local `ci/local_check.sh` script also gates the FDB tests: they run only when `FDB_CLUSTER_FILE` is set and points to an existing file; otherwise they are skipped with a message.
