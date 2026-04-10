# PR 03: Generate provider gtpacks and prepare GHCR publishing

    **Repository:** `greenticai/greentic-sorla-providers`

    ## Objective

    Implement pack generation for SoRLa providers so each provider can be packaged as a Greentic capability pack (gtpack), versioned, and published to GHCR.

    ## Why this PR exists

    The desired product model is pack-based. `greentic-sorla` packages should not hardcode provider code; they should bind to provider packs. This PR turns provider implementations into publishable Greentic units.

    ## Scope

    Build a provider pack generator that can package:
- provider metadata
- required runtime components
- configuration schema
- capability declarations
- compatibility metadata
Prepare OCI publishing flow, but it is fine if the first PR stops short of real release publication and instead produces the local pack artifacts and publishing workflow templates.

    ## Deliverables

    - provider pack manifest model
- per-provider pack generation
- local output structure for generated packs
- OCI publishing workflow/template docs
- version metadata included in packs
- examples showing one provider pack output

    ## Implementation notes for Codex

    Make pack generation generic across providers. FoundationDB, SharePoint placeholder, and RAG placeholder should all use the same pack generator pipeline. Ensure the output is consistent enough that future `greentic-sorla wizard` logic can consume provider catalog metadata and display pack choices.

    ## Acceptance criteria

    - At least one provider can be packaged locally as a gtpack artifact
- Pack manifests include capability declarations and version metadata
- Repo docs explain how provider packs are versioned/published
- The pack generator is not hardcoded to a single provider

    ## Non-goals

    - Fully working cloud release pipeline
- Real provider logic beyond what is needed to package examples

    ## Suggested files / areas to touch

    - `crates/sorla-provider-pack/`
- `docs/packs.md`
- `.github/workflows/release.yml`
- `examples/generated-packs/*`
