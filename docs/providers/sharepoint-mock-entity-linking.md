# SharePoint Mock Entity Linking

`provider-sharepoint-mock` validates generic external mapping documents and links deterministic SharePoint-style source references to generic ontology entities.

Core contracts stay generic. Fixture values such as `building_id` and `floor_id` can appear inside mock payloads or `metadata_json`, but they are not part of shared request types.

## Source References

The provider accepts deterministic source refs:

```text
sharepoint://tenant/{tenant}/document/{document_id}
sharepoint://tenant/{tenant}/list/{list_id}/item/{item_id}
```

Links resolve to `EvidenceDocument` entity refs in the tenant namespace.

## External Mapping

Valid mapping documents use schema `greentic.sorla.external-mapping.v1` and must target provider ID `greentic.sorla.provider.sharepoint-mock`.

Each mapping rule must include:

- `source_type`
- `target_concept`
- `id_field`
- `entity_fields`

The validator rejects empty mapping sets and malformed rule definitions.
