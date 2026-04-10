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

## Config Shape

Current config schema fields:

- `cluster_file`
- `tenant_prefix`

`cluster_file` is kept in the schema now because the intended production runtime remains FoundationDB-backed even though the current repo implementation uses a local transactional model for CI and development safety.

