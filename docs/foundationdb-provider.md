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
