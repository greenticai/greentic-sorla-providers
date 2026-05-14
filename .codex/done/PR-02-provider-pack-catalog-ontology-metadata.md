# PR 02 — Add ontology capability metadata to provider packs and catalog

## Repository

`greenticai/greentic-sorla-providers`

## Objective

Extend provider pack manifests and catalog entries so `greentic-sorx` and future `gtc` flows can select providers based on generic ontology capabilities.

## Manifest extension

Add an optional section to provider manifests:

```json
{
  "ontology_capabilities": {
    "schema": "greentic.sorla.provider.ontology-capabilities.v1",
    "supports_entity_read": true,
    "supports_entity_search": true,
    "supports_relationship_query": true,
    "supports_path_find": true,
    "supports_entity_linking": true,
    "supports_ontology_scoped_evidence": true,
    "supported_concept_types": ["*"],
    "supported_relationship_types": ["*"],
    "max_traversal_depth": 4,
    "supports_policy_context": false
  }
}
```

Use `*` only to mean provider can work generically from pack metadata. Provider-specific limitations should be explicit.

Implementation note for the current codebase: `ProviderPackManifest::from_metadata` is the single path used by the pack CLI. Add a typed optional `ontology_capabilities` field to `ProviderPackManifest`, and add the corresponding optional metadata either to `ProviderMetadata` or through an explicit manifest builder input. Keep `skip_serializing_if = "Option::is_none"` so existing JSON consumers remain compatible.

## Catalog extension

Extend catalog entries with:

```json
{
  "ontology": {
    "capabilities": ["entity-read", "relationship-query", "entity-link"],
    "max_traversal_depth": 4,
    "supports_generic_entity_refs": true
  }
}
```

## Requirements

1. Generated packs remain deterministic.
2. Existing providers get sensible capability metadata.
3. Provider catalog generation includes the new metadata from manifest input, not provider code loading.
4. No secrets or runtime credentials in metadata.
5. Existing catalog consumers should not break if the section is absent.
6. Update provider-local `catalog_entry()` helpers in `provider-foundationdb`, `provider-sharepoint-mock`, and `provider-rag-mock`; those helpers manually construct `ProviderCatalogEntry` today and will otherwise fail to compile once the catalog struct grows.
7. Regenerate `examples/generated-packs/**/manifest.json`, `examples/generated-packs/**/*.gtpack.json`, `examples/generated-packs/index.json`, and `examples/generated-catalog/provider-catalog.json` after the model change.

## Provider defaults

- `provider-foundationdb`: do not advertise relationship/query/path capabilities until PR 03 lands. It may include an ontology section with only `supports_generic_entity_refs: true` if backed by implemented behavior in PR 01.
- `provider-sharepoint-mock`: advertise external mapping/entity link capabilities only after PR 04 lands.
- `provider-rag-mock`: advertise ontology-scoped evidence/entity link capabilities only after PR 05 lands.

Avoid over-advertising unsupported capabilities.

## Tests

Add tests for:

- manifest serializes ontology metadata
- catalog serializes ontology metadata
- provider-specific capability metadata is deterministic
- pack generation still produces expected files
- generated catalog includes ontology capabilities

## Docs

Update:

- `docs/packs.md`
- `docs/catalog.md`
- `docs/ontology-provider-contract.md`

## Acceptance criteria

```bash
cargo test --all-features
cargo run -p sorla-provider-pack-cli
cargo run -p sorla-provider-catalog-cli
bash ci/local_check.sh
```
