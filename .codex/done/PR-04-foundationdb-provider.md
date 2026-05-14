# PR 04: Add FoundationDB SoRLa provider as an event-native gtpack

    **Repository:** `greenticai/greentic-sorla-providers`

    ## Objective

    Implement the first real provider: a FoundationDB-backed SoRLa provider that stores immutable events, materialized projections, and provider metadata using the shared SDK and can be packaged as a provider gtpack.

    ## Why this PR exists

    FoundationDB is the reference provider for the dynamic, event-native SoRLa model. It proves that SoRLa can evolve process logic rapidly without relying on mutable SQL schemas as the center of truth.

    ## Scope

    Implement a provider crate for FoundationDB covering:
- event append
- stream reads
- projection reads/writes
- replay/rebuild support hooks
- keyspace conventions
- provider config
- provider metadata
The implementation can begin with a local/dev-ready subset as long as the abstractions are clean and testable.

    ## Deliverables

    - `sorla-provider-foundationdb` crate
- keyspace layout documentation
- append/read implementation against FoundationDB
- projection state storage
- provider config schema
- gtpack generation for the provider
- integration tests against local/dev environment where feasible

    ## Implementation notes for Codex

    Design key prefixes clearly and document them. Recommended logical areas include:
- events
- projections
- indexes
- metadata
- checkpoints
- compatibility/version
Do not overbuild a full distributed runtime in this PR. The key goal is a clean, correct provider with enough capabilities to support the KAFD demo later. If some advanced indexing/rebuild capabilities need lightweight placeholders, document them clearly.

    ## Acceptance criteria

    - Provider can append and read events
- Provider can store/read projections
- Pack generation works for the FoundationDB provider
- Tests cover basic append/project/read behavior
- Keyspace layout is documented and not ad hoc

    ## Non-goals

    - SharePoint or RAG integration
- Full production-grade multi-region tuning
- Full observability stack

    ## Suggested files / areas to touch

    - `crates/sorla-provider-foundationdb/`
- `docs/foundationdb-provider.md`
- provider integration tests
