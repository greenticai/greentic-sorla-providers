# PR 06 — Add provider compatibility gates for ontology contracts

## Repository

`greenticai/greentic-sorla-providers`

## Objective

Introduce explicit compatibility gates so Sorx can know whether a provider can support a given ontology-enabled pack.

## Extend compatibility model

Add fields to `ContractCompatibility` or a new `OntologyContractCompatibility`:

```rust
pub struct OntologyContractCompatibility {
    pub supported_ontology_schema: String,
    pub supported_ontology_schema_range: String,
    pub supported_retrieval_binding_schema: Option<String>,
    pub supported_external_mapping_schema: Option<String>,
}
```

Current code note: `ContractCompatibility` currently contains only provider-contract and SoRLa IR fields, and `parses_semver_range()` validates only `supported_sorla_ir_range`. Add ontology compatibility as optional metadata so non-ontology providers can omit it, and add a separate parser/validator for ontology schema version ranges rather than overloading the IR range helper.

Coordinate this with PR 02: manifests and catalog entries will already have optional ontology metadata fields by then. This PR should add compatibility fields to those existing optional sections instead of introducing a second unrelated manifest/catalog shape.

## Requirements

1. Provider manifests include ontology compatibility when provider advertises ontology capabilities.
2. Catalog entries include compatibility ranges.
3. Compatibility range parsing is tested.
4. Providers that do not support ontology should explicitly omit or deny support.
5. Sorx should be able to use this metadata later without loading provider code.
6. Update the manual `catalog_entry()` constructors in all provider crates if `ProviderCatalogEntry` gains fields.
7. Regenerate example pack and catalog JSON after compatibility metadata is added.

## Tests

Add tests for:

- valid semver/schema range parsing
- invalid semver/schema range rejection
- provider with ontology support has compatibility metadata
- provider without ontology support does not falsely advertise support
- catalog generation preserves compatibility metadata

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
