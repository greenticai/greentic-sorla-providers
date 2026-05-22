# Provider Catalog

PR-07 turns generated provider manifests into a concrete discovery artifact for SoRLa wizard-style selection.

## Source of Truth

The provider catalog is generated from checked pack manifests, not from runtime discovery.

The current generation flow is:

1. generate provider packs with `cargo run -p sorla-provider-pack-cli`
2. generate the discovery catalog with `cargo run -p sorla-provider-catalog-cli`

This keeps the discovery model deterministic, inspectable, and usable in offline or local development flows.

## Output

By default the catalog generator writes:

- `examples/generated-catalog/provider-catalog.json`

Each catalog entry includes:

- provider ID
- provider version
- provider kind
- capability declarations
- tags
- mock or real marker
- status
- supported provider contract version
- supported SoRLa IR and IR range
- config schema path
- primary artifact URI
- optional OCI reference
- optional Rust provider SDK binding metadata
- optional ontology discovery metadata

Capability discovery starts with the top-level `capabilities` list. Newer providers may advertise canonical and projection-oriented flags such as `canonical-state`, `canonical-write`, `exact-index`, `composite-index`, `text-search-projection`, and `vector-search-projection`.

## Ontology Discovery Metadata

Catalog entries can include an optional `ontology` section projected from pack manifest `ontology_capabilities`.

The section contains:

- ontology capability declarations
- maximum traversal depth when relevant
- whether generic entity refs are supported
- ontology schema compatibility ranges
- optional retrieval binding or external mapping schema support
- optional structured index capability metadata
- optional structured search projection capability metadata

Catalog generation remains manifest-driven, so discovery tools can inspect ontology support without loading provider code.

## SDK Binding Metadata

When a provider pack includes `sdk_binding`, the generated catalog carries it through unchanged. This gives SORX or other runtime adapters a stable package/crate/contract tuple for selecting a provider crate instead of relying on provider IDs or OCI strings alone.

## Tagging

Current tags are intentionally simple and machine-friendly:

- provider kind tag such as `event-store`, `external-ref`, or `evidence`
- `mock` or `real`

That gives later wizard logic enough signal to filter compatible providers without overdesigning discovery early.
