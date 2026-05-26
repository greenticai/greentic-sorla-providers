# PR-01 — Add metric execution contracts to sorla-provider-core

Repository: `greenticai/greentic-sorla-providers`

## Goal

Add shared provider contracts for executing SoRLa metrics.

The provider workspace includes `crates/sorla-provider-core` for shared contracts. Add metric query/response/capability types there.

## Provider capability declarations

Add `ProviderCapability` enum variants using the repo's existing declaration style:
Rust enum variants serialized as kebab-case through `#[serde(rename_all = "kebab-case")]`.

Add variants equivalent to:

```text
metric-aggregate-count
metric-aggregate-sum
metric-aggregate-avg
metric-aggregate-min
metric-aggregate-max
metric-aggregate-distinct-count
metric-dimension-group-by
metric-time-bucket-hour
metric-time-bucket-day
metric-time-bucket-week
metric-time-bucket-month
metric-time-bucket-quarter
metric-time-bucket-year
metric-window-rolling
metric-formula-basic
```

Do not add a separate dotted string capability model such as `metrics.aggregate.count`
unless a compatibility requirement is documented. Pack and catalog metadata already
project `ProviderMetadata.capabilities: Vec<ProviderCapability>`.

## Query types

Add provider-neutral types in `sorla-provider-core`:

- `ProviderMetricQuery`
- `ProviderMetricSource`
- `ProviderMetricAggregation`
- `ProviderMetricFilter`
- `ProviderMetricFilterOperator`
- `ProviderMetricTimeBucket`
- `ProviderMetricDimension`
- `ProviderMetricResult`
- `ProviderMetricRow`
- `ProviderMetricValue`

The query contract must define the source explicitly. At minimum, support source
references for:

- named event stream, using the existing `EventStoreProvider` stream model
- canonical entity set, scoped by `SorNamespace` plus `entity_type`
- provider fixture source, for deterministic local/dev tests

Avoid commerce-specific or provider-specific fields in shared core types. Domain
values belong in payload JSON, metadata JSON, dimensions, or test fixtures.

Reuse existing generic ideas where possible:

- timestamps may follow the current provider-neutral string style used by `TimeRange`
- JSON payloads may use `serde_json::Value`, which is already a normal dependency
  of `sorla-provider-core`
- field paths should be provider-neutral strings, with docs explaining provider
  responsibility for resolving them against source payloads

## Trait

Add a synchronous trait matching the current provider style:

```rust
pub trait MetricProvider {
    fn query_metric(
        &self,
        query: ProviderMetricQuery,
    ) -> Result<ProviderMetricResult, ProviderError>;
}
```

Do not introduce async traits in this PR. Existing core provider traits are
synchronous.

## Error model

The existing shared error type is `ProviderError`, not `SorlaProviderError`.
Either extend `ProviderError` with metric-specific variants or map metric failures
onto existing variants with clear messages.

Preferred new variants:

```text
UnsupportedMetricCapability
UnknownMetricSource
UnknownMetricField
InvalidMetricFilter
MetricExecutionFailed
```

If adding variants is too large for this PR, use:

- `ProviderError::Unsupported("metric capability: ...")`
- `ProviderError::Validation("unknown metric source: ...")`
- `ProviderError::Validation("unknown metric field: ...")`
- `ProviderError::Validation("invalid metric filter: ...")`
- `ProviderError::NotImplemented("metric execution: ...")`

## Acceptance criteria

- Shared metric query/result types exist in `sorla-provider-core`.
- Providers can declare metric capabilities through `ProviderCapability`.
- Metric capability serialization is stable and kebab-case.
- The metric trait uses `ProviderError` and the repo's synchronous trait style.
- Query source semantics are explicit enough for PR-02 to implement without
  guessing whether to scan streams, entities, or fixtures.
- No concrete provider execution is required in this PR.
- Docs/comments explain provider responsibilities.
