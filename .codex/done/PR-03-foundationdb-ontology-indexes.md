# PR 03 — Extend FoundationDB provider with generic ontology indexes

## Repository

`greenticai/greentic-sorla-providers`

## Objective

Extend `provider-foundationdb` with generic entity and relationship index support.

Keep the current local/dev in-memory implementation, but design the keyspace layout so it maps cleanly to real FoundationDB later.

## Scope

Add support for:

- entity upsert/read/search
- relationship persist/query
- path finding over relationship indexes
- evidence link persistence/query
- ontology metadata storage
- stable namespace boundaries

Current code note: `FoundationDbProvider` is a local/dev in-memory provider with event streams and projections only. `KeyspaceLayout` currently has broad `indexes_prefix` and `metadata_prefix` fields, but no ontology-specific prefixes or entity/relationship state maps.

## Generic keyspace layout

Extend `KeyspaceLayout` with generic prefixes:

```text
{tenant_prefix}/ontology/model
{tenant_prefix}/entities/{entity_type}/{entity_id}
{tenant_prefix}/relationships/{relationship_type}/{from_type}/{from_id}/{to_type}/{to_id}
{tenant_prefix}/relationship-index/from/{from_type}/{from_id}/...
{tenant_prefix}/relationship-index/to/{to_type}/{to_id}/...
{tenant_prefix}/evidence-links/{entity_type}/{entity_id}/{evidence_id}
```

Do not use domain-specific key names.
Keep the existing event/projection/checkpoint prefixes intact, and add explicit ontology prefixes rather than reusing the generic `indexes_prefix` in tests.

## Implement traits

Implement the generic traits added in provider PR 01:

- `OntologyGraphProvider`
- `EntityStoreProvider`
- optionally `EntityLinkProvider` for persisted evidence links

## Types

Use generic `EntityRef`, `RelationshipInstance`, `RelationshipQuery`, `PathQuery`, and `OntologyPath`.
Use deterministic map/set types or explicit sort steps for all query results; the current provider uses `HashMap` internally, which is fine for storage but should not leak nondeterministic result order.

## Path finding

Implement deterministic bounded path search:

- max depth required
- stable ordering
- cycle-safe
- limit result count
- deterministic tie-breaking

## Tests

Add tests for:

- ontology keyspace layout prefixes remain unique and tenant-scoped
- entity insert/read
- relationship insert/query outgoing
- relationship insert/query incoming
- relationship query by type
- path finding depth 1
- path finding depth 2+
- cycle handling
- stable ordering
- evidence link persistence/query
- keyspace layout uniqueness

## Docs

Add:

- `docs/providers/foundationdb-ontology-indexes.md`

## Acceptance criteria

```bash
cargo test -p provider-foundationdb --all-features
cargo test --all-features
bash ci/local_check.sh
```
