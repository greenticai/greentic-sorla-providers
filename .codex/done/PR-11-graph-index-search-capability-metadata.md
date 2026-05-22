# PR: Add ontology graph, index and search capability metadata

Repo: `greenticai/greentic-sorla-providers`

## Goal
Expose provider metadata so SORX and setup tooling can determine whether a provider supports:

- canonical entity storage
- event storage
- graph traversal
- exact indexes
- composite indexes
- text search projection
- vector/evidence retrieval projection
- external mappings

## Current-code review corrections

- The repo already has provider capability metadata in two places:
  - top-level `ProviderMetadata.capabilities: Vec<ProviderCapability>`
  - optional `ProviderMetadata.ontology_capabilities: ProviderOntologyCapabilities`
- The existing ontology metadata schema is `greentic.sorla.provider.ontology-capabilities.v1`, serialized as `ontology_capabilities` in pack manifests and projected into `ontology` in the generated catalog.
- Generated pack manifests and the generated provider catalog already include the existing ontology metadata. This PR should extend those schemas rather than add a disconnected manifest object.
- Existing enum values include event/projection/entity/relationship/path/entity-link/external-mapping/evidence capabilities, but do not include `canonical_state`, exact indexes, composite indexes, text search projection, or vector projection.
- `ProviderCatalogOntology.capabilities` is already a searchable enum list for ontology support. Additional catalog fields should preserve this shape and add focused optional fields only where enum flags are not expressive enough.
- SORX deployment gating is not implemented in this repo. Acceptance should be limited to emitting metadata that downstream SORX tooling can consume later.

## Provider manifest extension
Prefer extending the existing metadata model unless there is a documented compatibility reason for a new schema:

- add new `ProviderCapability` enum variants for broad provider-level support
- add optional fields to `ProviderOntologyCapabilities` for graph/index/search details
- project those fields into `ProviderPackManifest` and `ProviderCatalogEntry`

Only add `greentic.sorla.provider.capabilities.v2` if the current top-level `capabilities` plus `ontology_capabilities` model cannot be evolved compatibly.

Example:

```json
{
  "schema": "greentic.sorla.provider.capabilities.v2",
  "canonical_state": true,
  "events": true,
  "ontology_graph": {
    "enabled": true,
    "max_depth_default": 3,
    "max_depth_hard_limit": 8
  },
  "indexes": {
    "exact": true,
    "composite": true
  },
  "search": {
    "text_projection": "optional",
    "vector_projection": "optional"
  }
}
```

If this JSON shape is used, map it explicitly to existing Rust types and generated manifest/catalog fields. Avoid emitting both this object and `ontology_capabilities` with divergent meanings.

## Acceptance criteria

- Generated provider pack manifests include the new metadata.
- Generated provider catalog includes searchable capability flags.
- Existing providers advertise only capabilities they actually implement today:
  - `provider-foundationdb`: event append/read, projections, entity read/search, relationship query, path find, entity link
  - `provider-sharepoint-mock`: external reference resolve, external mapping validate, entity link
  - `provider-rag-mock`: evidence query/resolve metadata and ontology-scoped evidence query
- Tests cover backward-compatible deserialization of existing `ontology_capabilities.v1` manifests.
- SORX can later gate deployment based on these emitted capabilities; no SORX runtime gating is required in this repo.
