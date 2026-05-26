# PR-04 — Add provider metrics fixtures, docs and compatibility tests

Repository: `greenticai/greentic-sorla-providers`

## Goal

Add fixtures, docs and compatibility tests for provider-side metric execution.

## Fixtures

Create deterministic seed data for a commerce-like scenario:

Records/events:

- click events
- visitors
- orders
- payments
- cost entries
- campaigns

Expected metrics:

- daily_clicks
- monthly_revenue
- monthly_cost
- conversion_rate dependency inputs
- campaign_roas dependency inputs

This scenario is fixture data only. Do not add commerce-specific fields, metric
names, or source types to `sorla-provider-core` shared contracts. Store domain
values in event payload JSON, canonical entity JSON, metadata JSON, dimensions,
or fixture files.

Seed fixtures through the source semantics defined in PR-01 and implemented in
PR-02. Prefer public provider APIs such as event append and canonical writes over
direct access to provider internals.

## Tests

Test provider contract behavior:

- provider declares metric capabilities
- provider executes supported aggregate metrics
- provider rejects unsupported metrics cleanly
- provider handles time bucketing
- provider handles dimensions
- provider handles filters

Also test repo-specific assumptions corrected during review:

- capabilities serialize as kebab-case `ProviderCapability` values
- providers without metric execution omit metric capabilities
- unsupported/unknown metric requests return `ProviderError`
- grouped result rows have deterministic ordering
- fixture seeding does not require private provider state access

## Docs

Add docs:

```text
docs/metrics-provider-contract.md
```

Cover:

- provider capabilities
- query contract
- result contract
- unsupported capability behavior
- memory/dev provider behavior
- FoundationDB notes
- catalog metadata

Docs should state that the current `provider-foundationdb` implementation is a
local/dev in-memory provider in this repo, despite the provider name. Real
FoundationDB client behavior is not assumed by these tests.

Docs should also state that this repo emits pack/catalog metadata for downstream
SoRLa/SORX consumers; it does not implement SORX doctor/start compatibility
checks.

## Acceptance criteria

- Provider metrics fixtures are deterministic.
- Provider contract tests pass.
- Unsupported providers return useful errors.
- Docs explain implementation expectations.
- Fixtures remain domain examples and do not change shared core contracts into a
  commerce-specific API.
- Compatibility tests cover generated pack/catalog metadata for one metric-capable
  provider and one provider without metric support.
