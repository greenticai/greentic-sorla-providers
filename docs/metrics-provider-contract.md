# Metrics Provider Contract

Provider-side metric execution is exposed through `MetricProvider` in
`sorla-provider-core`. The contract is intentionally provider-neutral: metric
queries name an explicit source, aggregate fields by provider-neutral field
paths, and return grouped rows with typed primitive values.

## Capabilities

Providers advertise metric support through the existing
`ProviderMetadata.capabilities` list. Metric capabilities serialize as kebab-case
`ProviderCapability` values, such as:

- `metric-aggregate-count`
- `metric-aggregate-sum`
- `metric-aggregate-avg`
- `metric-aggregate-min`
- `metric-aggregate-max`
- `metric-aggregate-distinct-count`
- `metric-dimension-group-by`
- `metric-time-bucket-day`
- `metric-time-bucket-month`

Absent metric capabilities mean unsupported. This repo does not emit a separate
negative capability list or a dotted metrics capability schema.

## Query Sources

`ProviderMetricQuery.source` makes the scan target explicit. The current shared
contract supports:

- `event-stream`: one named event stream from `EventStoreProvider`.
- `canonical-entities`: one canonical entity set scoped by `SorNamespace` and
  `entity_type`.
- `fixture`: deterministic local/dev fixture rows for tests and examples.

Concrete providers are responsible for resolving field paths against the source
payloads they own. The current local/dev FoundationDB provider resolves fields
from event payload JSON, canonical entity `data_json`, and selected record fields
such as `revision`, `created_at`, `updated_at`, `entity_id`, and `event_type`.

## Result Shape

`ProviderMetricResult.rows` contains deterministic grouped rows. Each row has:

- `dimensions`: a `BTreeMap<String, ProviderMetricValue>` for dimensions and
  time buckets.
- `metrics`: a `BTreeMap<String, ProviderMetricValue>` keyed by aggregation
  alias.

Numeric aggregate values are returned as `ProviderMetricValue::Number`. Missing
or non-numeric values are ignored for `sum`, `avg`, `min`, and `max`; if no
numeric values are present, those aggregates return `Null`. `count` counts rows
unless a field is provided, in which case null/missing values are skipped.
`distinct_count` skips null/missing values.

## Filters And Buckets

Filters run before aggregation. Supported operators are `equals`, `not_equals`,
`in`, `not_in`, `gt`, `gte`, `lt`, `lte`, `exists`, and `not_exists`.

Time buckets use string timestamps. The local/dev provider accepts stable
ISO-like timestamps such as `2026-05-01T09:00:00Z` and supports hour, day, week,
month, quarter, and year buckets. Day and month are covered by provider tests.

## Error Behavior

Metric-capable providers return `ProviderError` variants for unsupported or
invalid requests:

- `UnsupportedMetricCapability`
- `UnknownMetricSource`
- `UnknownMetricField`
- `InvalidMetricFilter`
- `MetricExecutionFailed`

Providers without metric execution should omit metric capabilities. Downstream
tools should check metadata before attempting metric execution.

## FoundationDB Notes

`provider-foundationdb` in this repository is a local/dev provider with an
in-memory transactional backing. It is named after the production provider family,
but these tests do not assume a real FoundationDB client.

The provider can execute metrics over:

- event streams seeded through `append_event`
- canonical entities seeded through `upsert_canonical_entity` or
  `apply_canonical_write`
- the deterministic `commerce` fixture source

The commerce fixture includes clicks, visitors, orders, payments, cost entries,
and campaigns. It is example data only; the shared core contract remains domain
neutral.

## Pack And Catalog Metadata

Generated provider packs and catalogs project metric support through the existing
`capabilities` field. This repository emits deterministic metadata for downstream
SoRLa/SORX consumers; it does not implement SORX doctor/start compatibility
checks.
