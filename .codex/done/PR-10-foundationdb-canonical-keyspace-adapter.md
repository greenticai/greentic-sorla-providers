# PR: Implement FoundationDB canonical keyspace adapter

Repo: `greenticai/greentic-sorla-providers`

## Goal
Implement a FoundationDB-backed provider adapter for canonical SORX persistence.

## Current-code review corrections

- `providers/provider-foundationdb` is currently a local/dev implementation with an in-memory transactional backing (`RwLock<InMemoryFoundationDb>`). It does not link to a FoundationDB client or perform real FoundationDB transactions yet.
- The current config is `FoundationDbConfig { cluster_file, tenant_prefix }`; there is no `tenant_id + sor_id` config or `/sorx/{tenant}/{sor}` keyspace today.
- `keyspace_layout()` currently exposes coarse prefixes under `tenant_prefix`, including events, projections, indexes, metadata, checkpoints, compatibility, ontology model, entities, relationships, relationship indexes, and evidence links.
- Existing writes are separate trait calls (`append_event`, `persist_projection`, `upsert_entity`, `upsert_relationship`, `upsert_evidence_link`). There is no single command API that atomically appends an event and updates projections, graph edges, indexes, and revision history together.
- `EntityStoreProvider` supports upsert/get/search only. There is no delete method in the core trait.
- Exact and composite indexes are metadata/prefix names only today; entity search scans in-memory records by type, namespace, text, and metadata substring.
- External source references are resolved by `provider-sharepoint-mock`; evidence retrieval is handled by `provider-rag-mock`. FoundationDB only stores evidence links, not full external reference or evidence records.

## Keyspace

Target canonical keyspace:

```text
/sorx/{tenant}/{sor}/meta/current_schema
/sorx/{tenant}/{sor}/meta/canonical_version
/sorx/{tenant}/{sor}/meta/active_views/{view_version}
/sorx/{tenant}/{sor}/entities/{entity_type}/{entity_id}
/sorx/{tenant}/{sor}/entity_versions/{entity_type}/{entity_id}/{revision}
/sorx/{tenant}/{sor}/events/{stream_id}/{sequence}
/sorx/{tenant}/{sor}/events_by_entity/{entity_type}/{entity_id}/{sequence}
/sorx/{tenant}/{sor}/events_by_time/{timestamp}/{event_id}
/sorx/{tenant}/{sor}/edges/out/{from_type}/{from_id}/{rel_type}/{to_type}/{to_id}
/sorx/{tenant}/{sor}/edges/in/{to_type}/{to_id}/{rel_type}/{from_type}/{from_id}
/sorx/{tenant}/{sor}/indexes/{entity_type}/{field}/{value}/{entity_id}
/sorx/{tenant}/{sor}/composite_indexes/{entity_type}/{index_name}/{encoded_values}/{entity_id}
/sorx/{tenant}/{sor}/external_refs/{source_type}/{source_id}
/sorx/{tenant}/{sor}/external_refs_by_entity/{entity_type}/{entity_id}/{source_type}/{source_id}
/sorx/{tenant}/{sor}/evidence/{evidence_id}
/sorx/{tenant}/{sor}/evidence_by_entity/{entity_type}/{entity_id}/{evidence_id}
/sorx/{tenant}/{sor}/migrations/{migration_id}
/sorx/{tenant}/{sor}/deployments/{deployment_id}
```

This is a replacement/expansion of the current `tenant_prefix` layout, not an existing layout. The PR must include a migration or compatibility decision for existing `tenant_prefix` config.

## Transaction behaviour
The desired canonical write path should atomically:

1. check idempotency key
2. append event
3. update canonical entity projection
4. update graph edges
5. update exact/composite indexes
6. update revision history

Because the current trait surface has no command/write-batch method, this PR must either:

- add a new contract for canonical atomic writes, or
- clearly document that existing separate trait methods remain non-atomic across method boundaries.

## Acceptance criteria

- Local/dev implementation may continue using an in-memory transactional backend if real FDB is not available, but it must expose the exact same provider contract.
- Key encoding is deterministic and documented.
- Includes tests for entity create/get/update/query. Add delete only if the core trait grows a delete operation.
- Includes tests for event append + projection update atomicity.
- Includes tests for reverse edge lookup.
- Includes tests proving the new `/sorx/{tenant}/{sor}` namespace does not depend on bundle/deployment version.
- Updates provider config schema, generated pack manifest, and generated catalog if config or capabilities change.
