# PR 01: Scaffold `greentic-sorla-providers` as a multi-provider gtpack family

    **Repository:** `greenticai/greentic-sorla-providers`

    ## Objective

    Create the `greentic-sorla-providers` repository as the canonical home for SoRLa provider implementations, shared provider contracts, pack generation, and GHCR publishing. The repo must be structured as a **provider family**, not a single-provider project.

    ## Why this PR exists

    The target architecture is not “a FoundationDB repo”. It is a provider collection similar in spirit to other Greentic provider families. FoundationDB is one provider. SharePoint placeholder, RAG placeholder, and future providers should fit naturally beside it.

    ## Scope

    Set up the workspace, docs, CI, release/publishing scaffolding, and a provider-family architecture baseline. The repo should clearly explain:
- shared provider SDK/contracts
- per-provider implementation crates
- per-provider gtpack generation
- OCI/GHCR publication
- discovery metadata for later selection by `greentic-sorla wizard`
Start with a clean internal structure that anticipates multiple providers.

    ## Deliverables

    - Rust workspace scaffold
- README and architecture docs
- crate layout for:
  - shared core/provider contracts
  - provider pack generation
  - provider catalog metadata
  - per-provider crates
- CI and release workflow
- placeholder provider list in docs

    ## Implementation notes for Codex

    Use naming and folder conventions that scale to many providers. Keep “provider family” explicit in docs. FoundationDB should not be placed at the architectural center of the repo; shared contracts should be. Add a small catalog format or placeholder metadata shape early so providers can later advertise capabilities and compatibility cleanly.

    ## Acceptance criteria

    - Repo builds in CI
- Docs clearly present the repo as a multi-provider family
- Workspace layout supports multiple provider crates
- There is a defined place for pack generation and provider catalog metadata
- No single provider dominates the architecture docs

    ## Non-goals

    - Actual provider implementations
- Full pack publishing
- Demo bindings

    ## Suggested files / areas to touch

    - `Cargo.toml`
- `crates/sorla-provider-core/`
- `crates/sorla-provider-pack/`
- `crates/sorla-provider-catalog/`
- `docs/architecture.md`
- `.github/workflows/*`
