# sorla-provider-core

Shared contracts and metadata for SoRLa providers.

The crate includes generic ontology-aware contracts for entity refs, relationship refs, ontology scopes, bounded path queries, entity links, external mapping validation, and ontology-scoped evidence filters. Shared request types are domain-agnostic; provider-specific domain values belong in `metadata_json` or fixture payloads.

It also defines canonical SORX source-of-record contracts. `SorNamespace` scopes production state by `tenant_id + sor_id`; optional `environment_id` is for non-production isolation and does not replace that production boundary. `CanonicalEntityRecord` and `SorEventRecord` carry revisioned canonical entity and event payloads with `serde_json::Value` data fields for durable store implementations.
