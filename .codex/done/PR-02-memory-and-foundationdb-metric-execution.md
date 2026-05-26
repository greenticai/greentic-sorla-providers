# PR-02 — Implement metric execution in memory/FoundationDB-capable providers

Repository: `greenticai/greentic-sorla-providers`

## Goal

Implement initial metric execution for available providers.

Start with the existing `providers/provider-foundationdb` local/dev provider. In
this repo it already uses an in-memory transactional backing (`InMemoryFoundationDb`)
for deterministic tests; it is not currently a real FoundationDB client.

Do not create a separate memory/dev provider unless the contract added in PR-01
cannot be implemented cleanly on `FoundationDbProvider`.

## MVP execution

Support:

```text
count
sum
avg
min
max
distinct_count
```

Support grouping by:

- time bucket
- dimensions

Support filters:

```text
equals
not_equals
in
not_in
gt
gte
lt
lte
exists
not_exists
```

Support time grains:

```text
hour
day
week
month
quarter
year
```

## Memory/dev provider

For local tests, implement metric execution on `FoundationDbProvider::for_tests()`
using deterministic seed data.

Seed data should be written through public provider APIs where possible:

- `EventStoreProvider::append_event` for named stream sources
- `CanonicalEntityStoreProvider::upsert_canonical_entity` or
  `CanonicalWriteProvider::apply_canonical_write` for canonical entity sources

If the PR-01 source contract introduces an explicit fixture source, keep fixture
loading provider-neutral and test-only. Do not reach into private provider state
from tests just to make aggregation possible.

## FoundationDB provider

`provider-foundationdb` has local/dev storage for events, canonical entities,
projections, graph relationships, and evidence links. It does not expose a generic
"scan all records/events" trait today.

Implement metric aggregation only for sources explicitly defined by PR-01. In
particular, define and test whether a query aggregates:

- one named event stream
- a canonical entity set scoped by `SorNamespace` and `entity_type`
- a deterministic fixture source

Unsupported or ambiguous sources must return clear errors rather than silently
scanning unrelated internal state.

Providers that do not implement `MetricProvider` should simply omit metric
capabilities. If a provider implements the trait but cannot support a requested
capability or source, return a clear `ProviderError` according to the PR-01 error
model.

For `provider-sharepoint-mock` and `provider-rag-mock`, do not add metric
execution in this PR unless they have a concrete metric source from PR-01. Tests
should prove their metadata does not advertise metric capabilities.

## Time and value semantics

Document and test:

- how timestamp fields are selected for time buckets
- how string timestamps are parsed and bucketed
- how missing/null/non-numeric values affect sum, avg, min, max, and filters
- deterministic output ordering for grouped rows
- whether `distinct_count` treats missing values as absent or a distinct value

## Acceptance criteria

- `FoundationDbProvider::for_tests()` can execute metric queries for the explicit
  source types added in PR-01.
- Providers without metric support do not advertise metric capabilities.
- Unsupported capabilities, unknown sources, unknown fields, and invalid filters
  return useful `ProviderError` values.
- Aggregation tests cover count, sum, avg and distinct_count.
- Time bucket tests cover day and month.
- Dimension group-by test covers at least one dimension.
- Filters are tested.
- Tests seed data through public provider APIs or a documented fixture source.
