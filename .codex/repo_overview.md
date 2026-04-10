# Repository Overview

## 1. High-Level Purpose
`greentic-sorla-providers` is a Rust workspace for building, packaging, and releasing SoRLa providers in the Greentic ecosystem.

It combines shared provider contracts, deterministic pack and catalog generation, concrete provider implementations, and repository automation. The repository now supports both crates.io publication for publishable Rust crates and OCI-style publication of generated provider pack and catalog artifacts to GHCR during tagged releases.

## 2. Main Components and Functionality

- **Path:** `crates/sorla-provider-core`
  - **Role:** Shared SoRLa provider contract crate.
  - **Key functionality:**
    - Defines provider metadata, capabilities, health/config validation, event append/read requests and responses, projection persistence/get/rebuild/checkpoint types, external reference resolution types, and evidence query types.
    - Exposes the traits implemented by concrete providers.
  - **Key dependencies / integration points:** Used by all provider crates and by the pack/catalog crates.

- **Path:** `crates/sorla-provider-pack`
  - **Role:** Deterministic local gtpack generation model and helpers.
  - **Key functionality:**
    - Defines canonical pack manifest structures.
    - Generates deterministic local pack outputs and metadata from provider descriptors.
    - Supports optional OCI reference fields used by release automation.
  - **Key dependencies / integration points:** Consumed by `sorla-provider-pack-cli` and aligned with provider metadata from `sorla-provider-core`.

- **Path:** `crates/sorla-provider-pack-cli`
  - **Role:** Pack generation CLI.
  - **Key functionality:**
    - Generates deterministic example packs under `examples/generated-packs/`.
  - **Key dependencies / integration points:** Uses `sorla-provider-pack` plus provider descriptors from the provider crates.

- **Path:** `crates/sorla-provider-catalog`
  - **Role:** Machine-readable provider catalog model.
  - **Key functionality:**
    - Defines catalog entries, compatibility metadata, tags, config schema paths, supported SoRLa IR ranges, and optional OCI references.
    - Builds catalog artifacts from generated pack/manifests.
  - **Key dependencies / integration points:** Consumed by `sorla-provider-catalog-cli` and aligned with the pack manifest shape.

- **Path:** `crates/sorla-provider-catalog-cli`
  - **Role:** Catalog generation CLI.
  - **Key functionality:**
    - Generates deterministic discovery output under `examples/generated-catalog/provider-catalog.json`.
  - **Key dependencies / integration points:** Uses `sorla-provider-catalog` and provider metadata.

- **Path:** `providers/provider-foundationdb`
  - **Role:** Local/dev event-native provider implementation.
  - **Key functionality:**
    - Implements immutable event append with expected-revision checks.
    - Implements event stream reads.
    - Persists and reads projections.
    - Supports checkpoint generation and deterministic replay from checkpoint or full stream.
    - Publishes provider metadata for pack/catalog generation.
  - **Key dependencies / integration points:** Implements traits from `sorla-provider-core`; feeds pack/catalog generation.

- **Path:** `providers/provider-sharepoint-mock`
  - **Role:** Deterministic mock external-reference provider.
  - **Key functionality:**
    - Resolves deterministic BTG, RFI, and site-visit object families.
    - Produces stable IDs, ordering, URLs, dates, and payloads for the same seed/config.
    - Validates `seed` and `tenant_id` config.
  - **Key dependencies / integration points:** Implements external-ref traits from `sorla-provider-core`; feeds pack/catalog generation.

- **Path:** `providers/provider-rag-mock`
  - **Role:** Deterministic mock evidence provider.
  - **Key functionality:**
    - Serves deterministic evidence items backed by seeded BTG, RFI, and site-visit sources.
    - Supports basic metadata faceting by `building_id`, `floor_id`, `document_type`, and `source_type`.
    - Validates `seed` and `max_results` config.
  - **Key dependencies / integration points:** Implements evidence-query traits from `sorla-provider-core`; feeds pack/catalog generation.

- **Path:** `ci/local_check.sh`
  - **Role:** Local and CI validation entrypoint.
  - **Key functionality:**
    - Runs formatting, clippy, tests, build, docs, and package/publish dry-run checks.
    - Detects publishable crates and validates package contents.
  - **Key dependencies / integration points:** Used by CI and publish workflows.

- **Path:** `ci/publish_oci_artifacts.sh`
  - **Role:** Release helper for OCI artifact publication.
  - **Key functionality:**
    - Regenerates provider packs and the catalog from source.
    - Logs into an OCI registry using `oras`.
    - Publishes each generated `*.gtpack.json` bundle plus its manifest and config schema to GHCR.
    - Publishes the generated provider catalog and pack index as a separate OCI artifact.
  - **Key dependencies / integration points:** Called by the release workflow; depends on `oras`, `sorla-provider-pack-cli`, and `sorla-provider-catalog-cli`.

- **Path:** `.github/workflows/ci.yml`
  - **Role:** Pull request and branch CI.
  - **Key functionality:**
    - Runs lint, tests, i18n checks, package dry-runs, perf smoke, and related quality gates.
  - **Key dependencies / integration points:** Calls `ci/local_check.sh` and the i18n tooling.

- **Path:** `.github/workflows/publish.yml`
  - **Role:** Release workflow.
  - **Key functionality:**
    - Verifies release tags match the workspace version.
    - Generates release pack and catalog artifacts.
    - Publishes crates.io artifacts for publishable crates after dry-run checks.
    - Publishes generated pack and catalog OCI artifacts to GHCR on tagged releases.
    - Attaches generated artifacts to the GitHub Release.
  - **Key dependencies / integration points:** Uses `ci/local_check.sh`, `ci/publish_oci_artifacts.sh`, ORAS, and GitHub release artifact actions.

- **Path:** `.github/workflows/perf.yml`
  - **Role:** Lightweight perf/concurrency CI.
  - **Key functionality:**
    - Runs tests plus criterion benchmark smoke for quick regression detection.

- **Path:** `.github/workflows/coverage-nightly.yml`
  - **Role:** Nightly coverage enforcement.
  - **Key functionality:**
    - Runs `greentic-dev coverage` and enforces `coverage-policy.json`.
    - Reads the LLVM coverage export from `target/coverage/coverage.json` and applies the repo's `global.line_coverage_min` threshold.

- **Path:** `docs/`
  - **Role:** Architecture and provider documentation.
  - **Key functionality:**
    - Documents workspace structure, pack generation, catalog generation, provider behavior, and OCI artifact publication layout.

## 3. Work In Progress, TODOs, and Stubs

- **Location:** `providers/provider-foundationdb`
  - **Status:** partial
  - **Short description:** Uses a transactional in-memory local/dev backend rather than a live FoundationDB backend; real remote/backend wiring is still deferred.

- **Location:** `i18n/*.json` non-English locale files
  - **Status:** stub
  - **Short description:** Locale files are scaffolded and validated structurally, but they still need true translations instead of English placeholders.

- **Location:** `benches/perf.rs`, `tests/perf_scaling.rs`, `tests/perf_timeout.rs`
  - **Status:** partial
  - **Short description:** Perf and concurrency harness exists, but workloads are still synthetic and should later be replaced with provider-realistic hot paths.

- **Location:** OCI artifact consumers
  - **Status:** partial
  - **Short description:** Release automation now publishes packs and catalog to GHCR, but downstream pull/install tooling is not yet present in this repo.

## 4. Broken, Failing, or Conflicting Areas

- **Location:** `providers/provider-foundationdb`
  - **Evidence:** Provider behavior is local/dev-safe and in-memory rather than backed by a live FoundationDB runtime.
  - **Likely cause / nature of issue:** The contract and semantics are implemented, but infrastructure-backed persistence is intentionally deferred to keep the first cut deterministic and CI-safe.

- **Location:** `ci/publish_oci_artifacts.sh`, `.github/workflows/publish.yml`
  - **Evidence:** OCI publication depends on GHCR credentials and ORAS availability during tagged releases.
  - **Likely cause / nature of issue:** The repository now automates OCI publication, but success still depends on GitHub release permissions and the external registry being available at release time.

- **Location:** `.github/workflows/coverage-nightly.yml`
  - **Evidence:** Coverage enforcement depends on the report location and JSON shape emitted by `greentic-dev coverage`.
  - **Likely cause / nature of issue:** The workflow has been aligned to the current `target/coverage/coverage.json` output and LLVM coverage JSON structure, but changes in upstream report format would require another workflow adjustment.

- **Location:** `tests/perf_scaling.rs`
  - **Evidence:** Scaling thresholds are intentionally permissive.
  - **Likely cause / nature of issue:** The current harness is a fast guardrail rather than a tight regression benchmark.

## 5. Notes for Future Work

- Add downstream consumption tooling or docs for pulling released provider OCI artifacts back into dev/runtime flows.
- Replace the local/dev FoundationDB backing model with a real backend path once operational constraints are defined.
- Tighten perf/concurrency checks around real provider hot paths instead of synthetic loops.
- Consider consolidating common metadata and identifiers with shared Greentic crates where the contracts are stable enough.
