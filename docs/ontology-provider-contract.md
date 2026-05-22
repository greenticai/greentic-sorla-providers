# Ontology Provider Contract

The ontology provider contract extends `sorla-provider-core` with domain-agnostic entity, relationship, path, linking, and ontology-scoped evidence types.

Core contracts use generic references such as `EntityRef`, `RelationshipRef`, and `OntologyScope`. Domain-specific fields such as building, floor, customer, account, or tenant identifiers belong in provider fixture payloads or `metadata_json`, not in shared request types.

## Generic Entity References

`EntityRef` identifies an ontology entity by type and ID, with optional namespace and version:

```rust
EntityRef {
    entity_type: "Contract".into(),
    entity_id: "contract-001".into(),
    namespace: Some("demo".into()),
    version: Some("v1".into()),
}
```

Relationships use `RelationshipRef` with generic `from` and `to` entity refs. Traversal uses `RelationshipDirection` and `RelationshipTraversalRule`.

## Canonical SORX State

Canonical source-of-record state uses `SorNamespace` rather than deployment or bundle identifiers:

```rust
SorNamespace {
    tenant_id: "tenant-a".into(),
    sor_id: "contracts".into(),
    environment_id: None,
}
```

Production state is scoped by `tenant_id + sor_id`. `environment_id` may isolate local, test, or staging data, but production canonical state must not be keyed by deployment version or generated pack version.

`CanonicalEntityRecord` stores revisioned canonical entity JSON and maps back to the existing `EntityRef` model through `SorNamespace::to_entity_namespace()`. `SorEventRecord` stores immutable canonical stream events with idempotency, actor, source-view, canonical-version, and JSON payload fields.

These records are FoundationDB-independent contracts. Durable providers can use them directly while still exposing the generic `EntityStoreProvider`, `EventStoreProvider`, and `ProjectionProvider` surfaces where useful.

## Ontology Scope

`OntologyScope` lets evidence, graph, and policy providers bind work to root entities and related traversal rules:

```rust
OntologyScope {
    root_entities: vec![customer_ref],
    include_related: vec![contract_traversal_rule],
    max_depth: Some(2),
    include_evidence_links: true,
}
```

## Evidence Filters

`EvidenceQueryFilter` is generic:

- `ontology_scope`
- `source_types`
- `document_types`
- `metadata_json`
- `time_range`
- `sensitivity_max`

Provider-specific fixture metadata may still include domain values, but callers should filter through ontology scope and generic metadata.

## External References

`ExternalReferenceRequest` uses `source_ref`, `metadata_json`, and optional ontology scope. Provider mocks may parse fixture metadata from `metadata_json`, but shared contracts do not expose domain fields.

## Traits

The core crate exposes synchronous traits matching the existing provider style:

- `EntityStoreProvider`
- `CanonicalEntityStoreProvider`
- `OntologyGraphProvider`
- `EntityLinkProvider`
- `ExternalMappingProvider`

Existing event, projection, external-reference, and evidence traits remain available.

## Pack And Catalog Metadata

Provider manifests may include optional ontology capability metadata under `ontology_capabilities` using schema `greentic.sorla.provider.ontology-capabilities.v1`.

Generated catalog entries project that manifest metadata into an optional `ontology` section so discovery tools can select providers by implemented generic ontology capabilities without loading provider code.

Ontology metadata includes compatibility gates for the ontology schema version range and optional retrieval-binding or external-mapping schemas. Providers that omit ontology metadata do not advertise ontology support.

Providers may also expose structured index and search projection metadata through `ontology_capabilities.index_capabilities` and `ontology_capabilities.search_capabilities`. These fields are optional for backward compatibility with existing v1 manifests. Broad provider-level support is still advertised through kebab-case `ProviderCapability` values such as `canonical-state`, `canonical-write`, `exact-index`, `composite-index`, `text-search-projection`, and `vector-search-projection`.
