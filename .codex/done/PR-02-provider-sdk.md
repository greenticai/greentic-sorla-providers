# PR 02: Implement shared SoRLa provider SDK and contracts

    **Repository:** `greenticai/greentic-sorla-providers`

    ## Objective

    Create the shared provider SDK that all SoRLa providers will implement. This SDK should hide common plumbing and standardize the capability surface for event storage, projection reads, external reference resolution, evidence linkage, health, and configuration.

    ## Why this PR exists

    Without a shared provider contract, every provider will drift. The SDK is the glue between `greentic-sorla` artifacts and provider-specific implementations. It is also the foundation for pack generation and provider conformance testing.

    ## Scope

    Define shared traits/types/contracts for:
- append event
- read stream
- read projection
- rebuild projection
- list/checkpoint projection status
- resolve external reference
- fetch evidence payload/metadata
- provider config and health
- capability/compatibility metadata
These contracts should map cleanly to Greentic capability-pack thinking but remain implementation-neutral.

    ## Deliverables

    - core provider trait definitions
- common request/response types
- provider error model
- provider metadata/catalog model
- basic test harness utilities
- docs explaining the expected behavior of any provider implementation

    ## Implementation notes for Codex

    Keep the SDK small but opinionated. It should be easy for providers to implement correctly. Define enough structure so that:
- FoundationDB can implement event/projection storage
- SharePoint placeholder can implement external ref resolution
- RAG placeholder can implement evidence lookup
Model requests in a way that works well with generated artifacts from `greentic-sorla`, especially provider requirement metadata and external source declarations.

    ## Acceptance criteria

    - Shared SDK compiles and has tests
- Contracts are documented and reasonably stable
- Placeholder/mock providers can be implemented against the SDK without hacks
- Error model is usable across providers
- Provider metadata shape can later feed pack generation/catalog discovery

    ## Non-goals

    - Actual FoundationDB logic
- Actual SharePoint or RAG logic
- GHCR publication

    ## Suggested files / areas to touch

    - `crates/sorla-provider-core/src/lib.rs`
- `crates/sorla-provider-core/src/types.rs`
- `crates/sorla-provider-core/src/traits.rs`
- `crates/sorla-provider-core/tests/*`
