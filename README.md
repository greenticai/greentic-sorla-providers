# greentic-sorla-providers

`greentic-sorla-providers` is the provider-family workspace for SoRLa integrations in the Greentic ecosystem.

This repository is intentionally organized as a multi-provider family, not as a single provider crate and not as a FoundationDB-only project. It now includes the locked shared contracts, deterministic pack and catalog generation, concrete provider implementations, and release automation for both crates.io and OCI artifacts.

## Workspace Layout

- `crates/sorla-provider-core`
  Shared SoRLa provider contracts, request/response types, error model, compatibility metadata, and provider capability declarations.

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
  Runs lint, tests, i18n validation, and packaging dry-runs on PRs and pushes to `main` or `master`.

- `.github/workflows/publish.yml`
  Validates the `v<version>` tag against Cargo metadata, runs local checks, publishes workspace crates to crates.io, publishes generated pack and catalog OCI artifacts to GHCR, and attaches generated outputs to the GitHub Release.

- `.github/workflows/perf.yml`
  Runs lightweight performance guardrails against the shared core crate.

- `.github/workflows/coverage-nightly.yml`
  Runs nightly coverage via `greentic-dev coverage` and enforces `coverage-policy.json`.

## Pack Generation

Run:

```bash
cargo run -p sorla-provider-pack-cli
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

## OCI Release Artifacts

Tagged releases publish generated provider artifacts in two places:

- GitHub Release assets:
  - `examples/generated-packs/index.json`
  - provider `*.gtpack.json`
  - provider-specific `*-manifest.json`
  - `examples/generated-catalog/provider-catalog.json`
- GHCR OCI artifacts:
  - `ghcr.io/<owner>/<repo>/<provider-id>-pack:vX.Y.Z`
  - `ghcr.io/<owner>/<repo>/provider-catalog:vX.Y.Z`

The release workflow regenerates pack and catalog outputs from source, validates the `v<version>` tag against the workspace version, publishes crates.io crates, then publishes the generated artifacts to GHCR using OCI artifact types rather than container images.

Required release secrets:

- `CARGO_REGISTRY_TOKEN` for crates.io publication
- `GITHUB_TOKEN` with `packages:write` permission for GHCR publication
