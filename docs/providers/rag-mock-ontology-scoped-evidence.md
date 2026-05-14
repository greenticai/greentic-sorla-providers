# RAG Mock Ontology-Scoped Evidence

`provider-rag-mock` is the deterministic evidence provider for generic ontology-scoped queries.

The provider uses `EvidenceQueryFilter.ontology_scope` plus generic `source_types` and `document_types`. Fixture-specific values such as `building_id` and `floor_id` remain in `metadata_json` and linked entity metadata rather than shared filter fields.

Each returned `EvidenceItem` includes:

- deterministic provenance
- linked generic entities
- relationship context
- optional permissions context JSON

Results are sorted by score descending and then stable evidence ID, and limits are applied after sorting.
