# PR 07: Add provider catalog metadata for wizard-driven discovery and selection

    **Repository:** `greenticai/greentic-sorla-providers`

    ## Objective

    Implement a provider catalog/discovery layer so `greentic-sorla` can later present compatible providers to the user via the wizard and bind SoRLa packages to provider gtpacks in a structured way.

    ## Why this PR exists

    The architecture should not force users to manually know every provider pack path or compatibility rule. The provider family needs a machine-readable catalog layer so SoRLa can discover:
- what provider categories exist
- what capabilities each pack offers
- which providers are mock/test-only
- which providers satisfy certain package requirements

    ## Scope

    Create a provider catalog model and generation flow that includes:
- provider ID
- pack reference/version
- capability declarations
- mock/real status
- compatibility metadata
- config schema references
- tags such as `event-store`, `external-ref`, `evidence`, `mock`
Integrate it with pack generation so providers automatically contribute catalog entries.

    ## Deliverables

    - provider catalog data model
- catalog generation from provider metadata
- docs explaining how discovery/selection should work
- sample catalog output including:
  - FoundationDB provider
  - SharePoint mock provider
  - RAG mock provider
- tests for catalog generation consistency

    ## Implementation notes for Codex

    Keep the catalog generic and machine-friendly. This is not a UI feature by itself, but it should later support wizard-driven provider selection in `greentic-sorla`. Include a clear marker for mock providers so the KAFD demo can intentionally bind to them during local/testing flows.

    ## Acceptance criteria

    - Catalog output contains all implemented providers
- Metadata clearly distinguishes mock vs non-mock providers
- Catalog shape is stable and documented
- Tests cover generation and compatibility metadata correctness

    ## Non-goals

    - Remote registry querying
- Full runtime binding logic in `greentic-sorla`
- UI presentation details

    ## Suggested files / areas to touch

    - `crates/sorla-provider-catalog/`
- `docs/catalog.md`
- sample generated catalog artifacts
