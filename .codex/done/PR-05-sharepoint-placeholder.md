# PR 05: Add SharePoint placeholder provider that generates deterministic fake external data

    **Repository:** `greenticai/greentic-sorla-providers`

    ## Objective

    Implement a SharePoint placeholder provider that behaves like an external system-of-record resolver but, for now, returns deterministic fake data for local testing and demos. Package it as a gtpack like any other provider.

    ## Why this PR exists

    The user explicitly asked for SharePoint to be a placeholder for now. We still need the end-to-end architecture, contracts, pack generation, and demo flows to work. A fake but deterministic provider is the best way to unblock the system while preserving the external-authoritative model.

    ## Scope

    Build a provider that:
- implements the shared external-ref contract
- accepts external pointer lookups
- returns deterministic fake records/documents/metadata
- can simulate BTGs, RFIs, and site visit records
- can attach stable IDs and provenance metadata
- is packaged as a gtpack
The fake data should be deterministic from input keys so tests and demos are stable.

    ## Deliverables

    - `sorla-provider-sharepoint-mock` or similar crate
- deterministic fake data generator
- support for a few object families:
  - BTG documents
  - RFI records
  - site visit notes
  - tenant/building metadata if needed
- provider config schema for mock mode
- gtpack generation
- tests validating deterministic outputs

    ## Implementation notes for Codex

    Do not pretend this is a real Microsoft Graph/SharePoint integration yet. Be explicit in docs and provider metadata that this is a mock provider. Design the request/response surface so a real SharePoint provider can later replace it with minimal contract churn. Make fake payloads realistic enough that the KAFD demo can show:
- building-specific documents
- page/section-like metadata
- stable source identifiers
- believable timestamps and provenance

    ## Acceptance criteria

    - Same external pointer request yields the same fake result every run
- Provider returns usable fake BTG/RFI/site visit structures
- Pack generation works
- Tests prove determinism and contract compliance
- Docs clearly state mock status and future replacement path

    ## Non-goals

    - Real SharePoint auth
- Microsoft Graph integration
- Complex permission mirroring

    ## Suggested files / areas to touch

    - `crates/sorla-provider-sharepoint-mock/`
- `docs/providers/sharepoint-mock.md`
- tests for deterministic fake outputs
