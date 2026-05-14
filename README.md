# greentic-sorla-providers

`greentic-sorla-providers` is the provider-family workspace for SoRLa integrations in the Greentic ecosystem.

This repository is intentionally organized as a multi-provider family, not as a single provider crate and not as a FoundationDB-only project. It now includes the locked shared contracts, deterministic pack and catalog generation, concrete provider implementations, and release automation for both crates.io and OCI artifacts.

## Workspace Layout

- `crates/sorla-provider-core`
  Shared SoRLa provider contracts, ontology-aware request/response types, error model, compatibility metadata, and provider capability declarations.

- `crates/sorla-provider-pack`
  Canonical local gtpack manifest model and deterministic local pack-generation helpers.

- `crates/sorla-provider-catalog`
  Catalog types and generation helpers for wizard-driven discovery based on generated pack manifests.

- `providers/provider-foundationdb`
  Local/dev event-native provider implementation wired to the shared contracts.

- `providers/provider-sharepoint-mock`
  Deterministic SharePoint-style mock external-reference provider wired to the shared contracts.

- `providers/provider-rag-mock`
  Deterministic RAG/evidence mock provider wired to the shared contracts.

## Architectural Decisions Locked In

- Provider-specific crates come after shared contracts, pack metadata, and catalog metadata are stable.
- SoRLa-specific interfaces live in `sorla-provider-core`.
- Pack metadata is generated locally first and acts as the source of truth for later OCI/GHCR publication.
- Catalog entries are generated from pack manifests, not discovered dynamically as the primary model.
- Provider implementations plug into the locked contract surface, pack metadata, and catalog metadata.

## CI and Releases

### Provider versions

Each provider is versioned independently. The source of truth is the provider crate's own `[package] version` in:

- `providers/provider-foundationdb/Cargo.toml`
- `providers/provider-sharepoint-mock/Cargo.toml`
- `providers/provider-rag-mock/Cargo.toml`

Do not use the workspace package version as a provider release version. Generated pack manifests, catalog entries, release metadata, and OCI tags read the selected provider crate version.

List provider versions:

```bash
cargo xtask provider-version list
```

Bump or set exactly one provider:

```bash
cargo xtask provider-version bump sharepoint-mock patch
cargo xtask provider-version set sharepoint-mock 0.1.5
```

These commands update only that provider's manifest version.

### Quick local checks

Run:

```bash
bash ci/local_check.sh
```

The script runs, in order:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all-features`
4. `cargo build --all-features`
5. `cargo doc --no-deps --all-features`
6. Packaging dry checks for each publishable crate:
   - `cargo package --no-verify`
   - `cargo package --allow-dirty`
   - `cargo publish --dry-run`

### i18n

Locale files are managed via `tools/i18n.sh`:

```bash
bash tools/i18n.sh all
bash tools/i18n.sh validate
bash tools/i18n.sh status
bash tools/i18n.sh translate
```

The translator expects flat key/value JSON in `i18n/en.json`. Locale files are currently scaffolded with English fallback strings until real translations are added.

### GitHub Actions

- `.github/workflows/ci.yml`
  Runs lint, tests, i18n validation, and packaging dry-runs on PRs and pushes to `main` or `master`. Provider checks are matrixed from `ci/provider-dependencies.json`, so a change below `providers/provider-sharepoint-mock/**` checks that provider, while shared crate, workflow, lockfile, or build-script changes check every affected provider.

- `.github/workflows/publish.yml`
  Publishes selected provider artifacts. Manual runs accept `provider` (`all` or one provider such as `sharepoint-mock`), `version_bump` (`none`, `patch`, `minor`, `major`), and optional `release_tag` for GitHub Release assets. A single-provider run builds and publishes only that provider. Tag pushes still publish all provider artifacts and release assets.

- `.github/workflows/perf.yml`
  Runs lightweight performance guardrails against the shared core crate.

- `.github/workflows/coverage-nightly.yml`
  Runs nightly coverage via `greentic-dev coverage` and enforces `coverage-policy.json`.

## Pack Generation

Run:

```bash
cargo run -p sorla-provider-pack-cli
cargo run -p sorla-provider-pack-cli -- --provider sharepoint-mock
```

This generates deterministic local pack artifacts in `examples/generated-packs/` for:

- FoundationDB
- SharePoint mock
- RAG mock

Each generated provider directory contains:

- `manifest.json`
- `*.gtpack.json`
- `schemas/provider-config.schema.json`

See `docs/packs.md` for the generated layout and OCI/GHCR publication flow.

## Catalog Generation

Run:

```bash
cargo run -p sorla-provider-catalog-cli
```

This generates a deterministic discovery artifact at `examples/generated-catalog/provider-catalog.json` from the checked-in provider pack manifests.

See `docs/catalog.md` for the catalog shape and discovery rationale.

## Ontology-Aware Provider Contracts

`sorla-provider-core` includes generic ontology types for entities, relationships, traversal, path finding, entity linking, external mapping validation, and ontology-scoped evidence queries.

The shared contract is domain-agnostic: fields such as building, floor, customer, account, and tenant identifiers belong in provider-specific metadata or fixtures, not in core request structs.

See `docs/ontology-provider-contract.md` for the core ontology types and trait surface.

## OCI Release Artifacts

Provider releases publish generated artifacts in two places:

- GitHub Release assets:
  - `examples/generated-packs/index.json`
  - provider `*.gtpack.json`
  - provider-specific `*-manifest.json`
  - `examples/generated-catalog/provider-catalog.json`
- GHCR OCI artifacts:
  - `ghcr.io/greenticai/sorla-providers/foundationdb:X.Y.Z`
  - `ghcr.io/greenticai/sorla-providers/sharepoint-mock:X.Y.Z`
  - `ghcr.io/greenticai/sorla-providers/rag-mock:X.Y.Z`

Exact semantic-version tags are the reliable reference mechanism for bundles and toolchain manifests:

```text
oci://ghcr.io/greenticai/sorla-providers/foundationdb:0.1.4
oci://ghcr.io/greenticai/sorla-providers/sharepoint-mock:0.1.4
oci://ghcr.io/greenticai/sorla-providers/rag-mock:0.1.4
```

Moving tags such as `latest` or `stable` may be added later, but bundles should use exact semantic versions.

The release workflow regenerates pack and catalog outputs from source, publishes crates.io crates on tag pushes, then publishes the selected generated artifacts to GHCR using OCI artifact types rather than container images.

For a manual republish of all OCI artifacts after this workflow is available on the selected branch:

```bash
gh workflow run publish.yml --ref main -f provider=all -f version_bump=none
```

To also upload GitHub Release assets from a manual run, pass a release tag:

```bash
gh workflow run publish.yml --ref main -f provider=all -f version_bump=none -f release_tag=v0.1.4
```

### Change detection

Provider rebuild selection is defined in `ci/provider-dependencies.json`:

- Files under `providers/<provider-name>/**` rebuild that provider.
- Shared crates such as `sorla-provider-core`, `sorla-provider-pack`, and `sorla-provider-catalog` rebuild every provider listed in that crate's `affects` array.
- Workflow, CI, xtask, root `Cargo.toml`, and `Cargo.lock` changes rebuild all providers.

When adding a provider or shared crate, update `ci/provider-dependencies.json` in the same change so CI and publish matrices stay accurate.

Required release secrets:

- `CARGO_REGISTRY_TOKEN` for crates.io publication
- `GITHUB_TOKEN` with `packages:write` permission for GHCR publication
