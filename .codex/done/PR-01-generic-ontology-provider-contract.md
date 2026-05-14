# PR 01 — Add generic ontology-aware provider contract

## Repository

`greenticai/greentic-sorla-providers`

## Objective

Extend `sorla-provider-core` with generic ontology-aware provider types and traits.

The contract must be **domain-agnostic**. Do not add fields such as `building_id`, `floor_id`, `tenant_id`, `customer_id`, `account_id`, etc. Domain fields belong in provider-specific metadata or fixture data, not core contracts.

## Add provider capabilities

Extend `ProviderCapability` with generic capabilities:

```rust
OntologyModelRead,
EntityRead,
EntitySearch,
RelationshipRead,
RelationshipQuery,
PathFind,
EntityLink,
SemanticAliasResolve,
ExternalMappingValidate,
OntologyScopedEvidenceQuery,
HybridEvidenceQuery,
PolicyContextResolve,
```

Use kebab-case serde names.

## Add generic references

Add:

```rust
pub struct EntityRef {
    pub entity_type: String,
    pub entity_id: String,
    pub namespace: Option<String>,
    pub version: Option<String>,
}

pub struct RelationshipRef {
    pub relationship_type: String,
    pub from: EntityRef,
    pub to: EntityRef,
}

pub enum RelationshipDirection {
    Incoming,
    Outgoing,
    Both,
}

pub struct RelationshipTraversalRule {
    pub relationship_type: Option<String>,
    pub direction: RelationshipDirection,
    pub max_depth: Option<u8>,
}
```

## Add ontology scope

```rust
pub struct OntologyScope {
    pub root_entities: Vec<EntityRef>,
    pub include_related: Vec<RelationshipTraversalRule>,
    pub max_depth: Option<u8>,
    pub include_evidence_links: bool,
}
```

## Replace domain-specific evidence filter shape

The current `EvidenceQueryFilter` contains domain-specific fields. Replace or deprecate those fields in favor of generic filters:

```rust
pub struct EvidenceQueryFilter {
    pub ontology_scope: Option<OntologyScope>,
    pub source_types: Vec<String>,
    pub document_types: Vec<String>,
    pub metadata_json: Option<String>,
    pub time_range: Option<TimeRange>,
    pub sensitivity_max: Option<String>,
}
```

If backwards compatibility is needed, keep old fields as deprecated optional fields behind a compatibility feature, but new providers and tests should use generic fields only.

## Replace domain-specific external reference request shape

The current `ExternalReferenceRequest` also contains domain-specific fields:

```rust
pub building_id: Option<String>,
pub floor_id: Option<String>,
```

Replace those with generic request metadata, so providers such as SharePoint mock can carry fixture-specific fields without making them part of the core contract:

```rust
pub struct ExternalReferenceRequest {
    pub reference_type: String,
    pub reference_id: String,
    pub source_ref: Option<String>,
    pub metadata_json: Option<String>,
    pub ontology_scope: Option<OntologyScope>,
}
```

If a compatibility path is needed, gate the old fields the same way as legacy evidence filters. New tests and docs must not construct core requests with `building_id` or `floor_id`.

## Add entity-linking types

```rust
pub struct EntityLink {
    pub entity: EntityRef,
    pub source_ref: String,
    pub evidence_id: Option<String>,
    pub confidence: f32,
    pub match_kind: String,
    pub provenance: String,
    pub metadata_json: Option<String>,
}

pub struct EntityLinkRequest {
    pub source_ref: Option<String>,
    pub evidence_id: Option<String>,
    pub content_json: Option<String>,
    pub candidate_types: Vec<String>,
    pub ontology_scope: Option<OntologyScope>,
}
```

## Add provider traits

```rust
pub trait OntologyGraphProvider {
    fn query_relationships(
        &self,
        request: RelationshipQuery,
    ) -> Result<Vec<RelationshipInstance>, ProviderError>;

    fn find_paths(
        &self,
        request: PathQuery,
    ) -> Result<Vec<OntologyPath>, ProviderError>;
}

pub trait EntityStoreProvider {
    fn upsert_entity(&self, entity: EntityRecord) -> Result<EntityRecord, ProviderError>;
    fn get_entity(&self, entity: EntityRef) -> Result<Option<EntityRecord>, ProviderError>;
    fn search_entities(&self, request: EntitySearchQuery) -> Result<Vec<EntityRecord>, ProviderError>;
}

pub trait EntityLinkProvider {
    fn link_entities(
        &self,
        request: EntityLinkRequest,
    ) -> Result<Vec<EntityLink>, ProviderError>;
}

pub trait ExternalMappingProvider {
    fn validate_mapping(&self, mapping_json: &str) -> Result<(), ProviderError>;
}
```

## Add types

- `RelationshipQuery`
- `RelationshipInstance`
- `EntityRecord`
- `EntitySearchQuery`
- `PathQuery`
- `OntologyPath`
- `OntologyPathStep`
- `TimeRange`
- `PolicyContextRequest`
- `PolicyContext`

## Requirements

1. All new types derive `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`.
2. Keep `#![forbid(unsafe_code)]`.
3. Avoid async traits unless the repo already uses them.
4. Preserve existing provider tests or update them safely.
5. Add compatibility notes if evidence filter shape changes.
6. Add examples using generic entity refs only.
7. Update `crates/sorla-provider-core/src/lib.rs` re-exports for every new type and trait.
8. Update all current tests and providers that construct `EvidenceQueryFilter` or `ExternalReferenceRequest`.

## Tests

Add tests for:

- serde roundtrip for `EntityRef`
- serde roundtrip for `OntologyScope`
- serde roundtrip for generic `EvidenceQueryFilter`
- serde roundtrip for generic `ExternalReferenceRequest`
- capability presence checks
- path query serialization
- entity record/search serialization
- entity link serialization

## Docs

Add:

- `docs/ontology-provider-contract.md`
- update `README.md`

## Acceptance criteria

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
bash ci/local_check.sh
```
