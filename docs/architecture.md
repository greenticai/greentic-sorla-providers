# SoRLa Provider Family Architecture

This workspace is split into three layers.

## Shared Contracts

`crates/sorla-provider-core` defines the SoRLa-specific provider contract surface:

- provider identity and compatibility metadata
- provider health and config validation
- event append and event stream read
- projection get, rebuild, and checkpoint flow
- external reference resolution
- evidence query and evidence resolution
- pack metadata emission hook inputs

These are intentionally local to SoRLa because they represent SoRLa-specific behavior, not generic Greentic behavior.

## Packaging and Discovery

`crates/sorla-provider-pack` converts shared provider metadata into a deterministic local pack manifest shape.

`crates/sorla-provider-catalog` converts generated pack manifests into a machine-readable provider catalog that later wizard flows can consume.

The catalog source of truth is pack-generation output, not runtime discovery.

## Provider Implementations

Concrete providers live under `providers/` and compile against `sorla-provider-core`.

Current provider crates are stubs:

- `provider-foundationdb`
- `provider-sharepoint-mock`
- `provider-rag-mock`

Real provider behavior should be added only after contracts, pack manifests, and catalog schemas are stable.

