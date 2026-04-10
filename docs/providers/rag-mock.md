# RAG Mock Provider

PR-06 introduces a deterministic evidence provider for local development and demo flows.

## Current Scope

The provider does not use real embeddings, vector search, or document ingestion.

Instead it implements the shared evidence contract and returns deterministic evidence results from seeded mock source documents covering:

- BTG sources
- RFI sources
- site-visit sources

## Evidence Shape

Each result uses the locked shared `EvidenceItem` shape:

- `evidence_id`
- `source_type`
- `source_ref`
- `document_id`
- `section_id`
- `page`
- `chunk_id`
- `snippet`
- `score`
- `provenance`
- `metadata_json`

## Filtering

The provider supports basic metadata faceting now:

- `building_id`
- `floor_id`
- `document_type`
- `source_type`

This keeps the contract useful for demo flows without overcommitting to an advanced search API too early.

## Config Shape

Current config fields:

- `seed`
- `max_results`

The `seed` anchors deterministic evidence generation. `max_results` caps query output while the query-level `limit` can request a smaller number.

