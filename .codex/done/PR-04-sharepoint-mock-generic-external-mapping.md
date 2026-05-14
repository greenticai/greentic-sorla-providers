# PR 04 — Make SharePoint mock support generic external mapping and entity linking

## Repository

`greenticai/greentic-sorla-providers`

## Objective

Extend `provider-sharepoint-mock` so it can act as a deterministic external-reference provider for generic ontology entities.

Do not expose building/floor concepts in the core provider contract. It is fine for mock seed data to contain domain-specific metadata inside `metadata_json`.

## Add support for

- `ExternalMappingProvider`
- `EntityLinkProvider`
- generic `EntityRef`
- generic metadata-based linking
- deterministic source refs

Current code note: `provider-sharepoint-mock` currently implements `ExternalReferenceProvider` using `ExternalReferenceRequest { reference_type, reference_id, building_id, floor_id }`, and its seed record structs expose `building_id` and `floor_id` as typed fields. After PR 01, migrate request handling to `source_ref`, `metadata_json`, and generic ontology types. Fixture records may still contain domain-specific metadata, but it should live inside serialized mock payload/metadata rather than the core request contract.

## Generic source reference shape

Use source refs like:

```text
sharepoint://tenant/{tenant}/document/{document_id}
sharepoint://tenant/{tenant}/list/{list_id}/item/{item_id}
```

## Mapping example

The provider should accept mapping JSON such as:

```json
{
  "schema": "greentic.sorla.external-mapping.v1",
  "provider_id": "greentic.sorla.provider.sharepoint-mock",
  "mappings": [
    {
      "source_type": "document",
      "target_concept": "EvidenceDocument",
      "id_field": "document_id",
      "entity_fields": {
        "title": "title",
        "source_url": "source_url"
      }
    }
  ]
}
```

## Entity linking behavior

Given content/external reference metadata, return generic links:

```json
{
  "entity": {
    "entity_type": "EvidenceDocument",
    "entity_id": "doc-123"
  },
  "source_ref": "sharepoint://tenant/demo/document/doc-123",
  "confidence": 1.0,
  "match_kind": "external-id",
  "provenance": "sharepoint-mock deterministic mapping"
}
```

When converting existing deterministic records, use stable IDs derived from existing mock record IDs:

- BTG document -> `EvidenceDocument`
- RFI record -> `EvidenceDocument` or a provider-specific fixture concept carried in metadata
- Site visit record -> `EvidenceDocument` or a provider-specific fixture concept carried in metadata

Do not require a `building_id` or `floor_id` to generate a link. If those values appear in seed data, put them in `metadata_json`.

## Tests

Add tests for:

- valid mapping validation
- invalid mapping rejection
- deterministic external reference resolution
- external reference resolution from the new generic `ExternalReferenceRequest`
- generic entity link generation
- no core dependency on building/floor fields
- metadata_json can carry arbitrary domain-specific fields

## Docs

Add:

- `docs/providers/sharepoint-mock-entity-linking.md`

## Acceptance criteria

```bash
cargo test -p provider-sharepoint-mock --all-features
cargo test --all-features
bash ci/local_check.sh
```
