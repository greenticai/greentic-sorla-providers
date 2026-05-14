# PR 07 — Add deterministic ontology provider scenario

## Repository

`greenticai/greentic-sorla-providers`

## Objective

Add a provider-local deterministic smoke scenario proving that the ontology provider stack works across the crates in this repo.

Do not depend on `greentic-sorla` or `greentic-sorx` commands. This PR should exercise the local provider contracts, provider implementations, pack generation, and catalog generation.

## Scenario requirements

Use a generic business-domain fixture:

```text
Customer
Supplier
Contract
Asset
Obligation
EvidenceDocument
```

Relationships:

```text
Customer has_contract Contract
Contract governs Asset
EvidenceDocument supports Contract
Supplier fulfils_obligation Obligation
```

## Provider-local flow

1. `provider-foundationdb` stores generic entities and relationships.
2. `provider-foundationdb` finds a deterministic path from `Customer` to `EvidenceDocument`.
3. `provider-sharepoint-mock` validates a generic external mapping.
4. `provider-sharepoint-mock` links a deterministic source ref to `EvidenceDocument`.
5. `provider-rag-mock` returns ontology-scoped evidence for a generic entity.
6. `sorla-provider-pack-cli` emits deterministic provider manifests.
7. `sorla-provider-catalog-cli` emits deterministic catalog metadata.

## Add entrypoint

Add a provider-local smoke entrypoint:

```bash
cargo xtask ontology-smoke
```

The smoke must be CI-safe, deterministic, and must not require network services or real FoundationDB.

## Tests

Add tests for:

- deterministic ontology fixture loading
- deterministic path query output
- deterministic external mapping validation
- deterministic entity link output
- ontology-scoped evidence output
- generated pack/catalog metadata checks

## Docs

Add:

- `docs/providers/ontology-smoke-scenario.md`

## Acceptance criteria

```bash
cargo xtask ontology-smoke
cargo test --all-features
cargo run -p sorla-provider-pack-cli
cargo run -p sorla-provider-catalog-cli
bash ci/local_check.sh
```
