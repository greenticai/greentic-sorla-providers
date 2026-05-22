# PR: Add canonical SORX ontology persistence contracts

Repo: `greenticai/greentic-sorla-providers`

## Goal
Add shared provider contracts for a real SORX canonical system of record backed by FoundationDB or compatible durable stores.

## Current-code review corrections

- The repo already exposes generic SoRLa contracts in `crates/sorla-provider-core`:
  - `EventStoreProvider`
  - `ProjectionProvider`
  - `EntityStoreProvider`
  - `OntologyGraphProvider`
  - `EntityLinkProvider`
  - `ExternalMappingProvider`
  - `EvidenceProvider`
- Do not add parallel traits with new names unless there is a documented contract break. Prefer extending the existing generic traits and `ProviderCapability` enum, or adding narrowly-scoped new request/record types that those traits can use.
- `EvidenceProvider` currently has `query_evidence`; there is a `ProviderCapability::EvidenceResolve` capability but no resolve method yet.
- There is no `IndexProvider` or `SearchProjectionProvider` contract today. If exact/composite index or search projection support is required, add explicit capability metadata first and keep the implementation optional.
- `EntityRef` already carries `namespace` and `version`. A new `SorNamespace` must define how it maps to existing `EntityRef.namespace` and provider config, instead of introducing a second unrelated namespace model.
- Core record payloads currently use `String` JSON fields (`payload`, `state_json`, `metadata_json`) rather than `serde_json::Value`. If this PR switches to `serde_json::Value`, promote `serde_json` from a dev dependency to a normal dependency of `sorla-provider-core` and update all generated manifest/catalog serialization tests.

## Required contracts
Add or extend the existing contracts for:

- `EventStoreProvider`
- `ProjectionProvider`
- `EntityStoreProvider`
- `OntologyGraphProvider`
- `EntityLinkProvider`
- `ExternalMappingProvider`
- `EvidenceProvider`

Add new contracts only where the current surface cannot represent the behavior:

- exact/composite index capability metadata
- text/vector search projection capability metadata
- optional evidence resolve method if `EvidenceResolve` should become executable rather than metadata-only

## Key design rule
Production state is scoped by:

```text
tenant_id + sor_id
```

not by bundle version or deployment version.

`environment_id` may be part of local/dev isolation or non-production staging, but it must not replace `tenant_id + sor_id` as the production state boundary.

## Types to add or adapt

```rust
pub struct SorNamespace {
    pub tenant_id: String,
    pub sor_id: String,
    pub environment_id: Option<String>,
}

pub struct CanonicalEntityRecord {
    pub entity_type: String,
    pub entity_id: String,
    pub canonical_version: String,
    pub revision: u64,
    pub data_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

pub struct SorEventRecord {
    pub event_id: String,
    pub stream_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub entity_ref: EntityRef,
    pub command_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub actor: Option<String>,
    pub source_view_version: Option<String>,
    pub canonical_version: String,
    pub payload_json: serde_json::Value,
    pub timestamp: String,
}
```

When adapting current types:

- decide whether `CanonicalEntityRecord` replaces or wraps `EntityRecord`
- decide whether `SorEventRecord` replaces or wraps `EventRecord`
- preserve deterministic ordering and stable JSON output for pack/catalog generation
- keep contracts independent of FoundationDB-specific APIs

## Acceptance criteria

- Contracts compile independently of FoundationDB.
- Existing provider mocks still compile.
- New contracts include deterministic serialisation tests.
- Existing tests around `ProviderMetadata`, pack manifests, and catalog generation are updated for any new capability fields.
- Docs explain shared canonical state, multiple views, and why deployment-isolated state is not production SoR.
