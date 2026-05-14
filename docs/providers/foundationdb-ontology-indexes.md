# FoundationDB Ontology Indexes

`provider-foundationdb` includes local/dev in-memory generic ontology indexes that map cleanly to future FoundationDB keyspaces.

The provider supports:

- generic entity upsert, read, and search
- generic relationship upsert and query
- deterministic bounded path finding
- persisted evidence links
- tenant-scoped ontology keyspace prefixes

The current implementation remains in-memory for local development and tests. It preserves deterministic result ordering so generated tests and smoke scenarios do not depend on `HashMap` iteration order.

## Keyspace Layout

The ontology prefixes are tenant-scoped:

```text
{tenant_prefix}/ontology/model
{tenant_prefix}/entities
{tenant_prefix}/relationships
{tenant_prefix}/relationship-index/from
{tenant_prefix}/relationship-index/to
{tenant_prefix}/evidence-links
```

These prefixes sit alongside the existing event, projection, checkpoint, metadata, and compatibility prefixes.
