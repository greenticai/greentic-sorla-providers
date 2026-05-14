# PR 05 — Make RAG mock use generic ontology-scoped evidence queries

## Repository

`greenticai/greentic-sorla-providers`

## Objective

Refactor `provider-rag-mock` to use generic ontology-scoped evidence queries instead of domain-specific evidence filters.

The RAG mock should become the deterministic test provider for hybrid ontology + evidence retrieval.

## Replace domain-specific filter usage

Move away from filters like:

```rust
building_id
floor_id
```

Use:

```rust
EvidenceQueryFilter {
    ontology_scope: Some(OntologyScope {
        root_entities: vec![EntityRef { ... }],
        include_related: vec![RelationshipTraversalRule { ... }],
        max_depth: Some(2),
        include_evidence_links: true,
    }),
    source_types: vec![],
    document_types: vec![],
    metadata_json: None,
    time_range: None,
    sensitivity_max: None,
}
```

Current code note: `provider-rag-mock` currently filters `RagSeedDocument` by `building_id`, `floor_id`, singular `document_type`, and singular `source_type`. PR 01 changes `EvidenceQueryFilter` to generic fields with plural `source_types` and `document_types`; update all direct struct construction in the RAG tests when this lands.

## Evidence item extensions

Extend `EvidenceItem` or add a companion struct to include:

```rust
pub linked_entities: Vec<EntityLink>
pub relationship_context: Vec<RelationshipRef>
pub permissions_context_json: Option<String>
```

If changing `EvidenceItem` is breaking, add `EvidenceEnvelope` while keeping the old item available.
Because `EvidenceItem` already derives `PartialEq` but not `Eq` due to `score: f32`, any envelope that includes scores should follow the same derive constraints.

## Query behavior

The mock should:

1. Accept free-text query.
2. Accept generic ontology scope.
3. Return deterministic evidence.
4. Attach linked generic entities.
5. Preserve provenance.
6. Sort by score then stable ID.
7. Respect limits.

For the current seed data, convert `building_id` and `floor_id` into generic linked entities in `metadata_json`/`linked_entities` rather than keeping them as filter fields. Tests may still assert fixture metadata contains those values, but generic ontology-scope filters should drive matching.

## Tests

Add tests for:

- ontology-scoped query returns deterministic results
- entity scoped query filters evidence
- relationship scoped query filters evidence
- old domain-specific filter tests are removed or moved to fixture-specific metadata tests
- linked entities are present
- provenance is present
- limits are respected

## Docs

Add:

- `docs/providers/rag-mock-ontology-scoped-evidence.md`

## Acceptance criteria

```bash
cargo test -p provider-rag-mock --all-features
cargo test --all-features
bash ci/local_check.sh
```
