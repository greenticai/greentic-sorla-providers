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
- `OntologyGraphProvider`
- `EntityLinkProvider`
- `ExternalMappingProvider`

Existing event, projection, external-reference, and evidence traits remain available.

## Pack And Catalog Metadata

Provider manifests may include optional ontology capability metadata under `ontology_capabilities` using schema `greentic.sorla.provider.ontology-capabilities.v1`.

Generated catalog entries project that manifest metadata into an optional `ontology` section so discovery tools can select providers by implemented generic ontology capabilities without loading provider code.

Ontology metadata includes compatibility gates for the ontology schema version range and optional retrieval-binding or external-mapping schemas. Providers that omit ontology metadata do not advertise ontology support.
