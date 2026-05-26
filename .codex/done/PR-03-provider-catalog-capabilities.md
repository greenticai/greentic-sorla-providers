# PR-03 — Publish provider metric capabilities in pack/catalog metadata

Repository: `greenticai/greentic-sorla-providers`

## Goal

Expose metric capabilities in provider pack/catalog metadata so SoRLa/SORX can determine whether a provider can execute a metric definition.

## Capability metadata

Provider manifests/catalog entries already include supported capabilities through
`capabilities: Vec<ProviderCapability>`. Metric support should be emitted through
the same field after PR-01 adds metric enum variants.

Example serialized shape:

```json
{
  "capabilities": [
    "metric-aggregate-count",
    "metric-aggregate-sum",
    "metric-time-bucket-day",
    "metric-time-bucket-month",
    "metric-dimension-group-by"
  ]
}
```

Do not add dotted capability strings or a second capability schema unless a
backward-compatibility requirement is documented.

## Catalog behavior

When generating catalogs:

- include metric capabilities for providers that actually support them
- treat absent metric capabilities as unsupported
- keep ordering deterministic
- preserve independent provider versioning behavior

The current model lists supported capabilities only. Do not add a negative or
"unsupported capabilities" list in this PR unless downstream tooling has a
specific need for it.

If PR-01 adds structured metric metadata beyond enum flags, add it as an optional
field on the existing provider metadata model and project it through pack and
catalog generation. Keep it backward-compatible for existing manifests.

## SoRLa/SORX use

This metadata should allow:

- SoRLa pack validation to declare required metric capabilities
- downstream SORX doctor/start flows to check selected provider compatibility
- Designer/prompt flows to suggest provider requirements

Do not implement SORX doctor/start compatibility checks in this repository. This
repo's responsibility is to emit deterministic provider metadata that downstream
tooling can consume.

## Acceptance criteria

- Provider pack manifests include metric capabilities through the existing
  `capabilities` field.
- Provider catalog entries include metric capabilities through the existing
  `capabilities` field.
- Generated artifacts are deterministic.
- Tests cover `provider-foundationdb` with metric support and at least one
  provider without metric support.
- Tests prove providers without metric execution omit metric capabilities.
- Existing generated examples are updated only if the corresponding provider
  metadata changes.
