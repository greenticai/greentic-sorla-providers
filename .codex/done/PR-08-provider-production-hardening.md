# PR 08 — Harden provider ontology metadata and local checks

## Repository

`greenticai/greentic-sorla-providers`

## Objective

Harden the provider-side ontology metadata, generated artifacts, and local checks after PR 01 through PR 07.

This PR should not add cross-repo functionality. It should make this repo's provider contracts, generated pack/catalog outputs, and smoke scenario safer and easier to operate.

## Hardening areas

### 1. Schema versioning

Verify provider-owned schema strings are explicit and tested:

```text
greentic.sorla.provider.ontology-capabilities.v1
greentic.sorla.ontology.v1
greentic.sorla.ontology.graph.v1
greentic.sorla.retrieval-bindings.v1
greentic.sorla.external-mapping.v1
```

### 2. Compatibility rules

Add or extend tests so provider metadata:

- accepts known-compatible ontology schema ranges
- rejects invalid schema ranges
- omits ontology metadata for providers without ontology support
- preserves compatibility metadata through generated pack and catalog JSON

### 3. Determinism

Verify:

- generated pack JSON is stable
- generated catalog JSON is stable
- ontology smoke output is stable
- provider query results are sorted deterministically

### 4. Security

Add provider-local checks for:

- no secrets in generated pack/catalog artifacts
- no credential-like keys in provider ontology metadata
- sensitivity/permissions context is present for RAG mock evidence

### 5. CI

Extend `bash ci/local_check.sh` to run the provider-local ontology smoke from PR 07.

## Docs

Add:

- `docs/ontology-production-readiness.md`
- `docs/ontology-security.md`
- `docs/ontology-compatibility.md`

## Acceptance criteria

```bash
cargo xtask ontology-smoke
bash ci/local_check.sh
```
