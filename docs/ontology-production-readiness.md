# Ontology Production Readiness

Provider-side ontology readiness is covered by local contracts, generated metadata, and the smoke command:

```bash
cargo xtask ontology-smoke
```

The smoke runs without external services and verifies graph traversal, external mapping, entity linking, ontology-scoped evidence, generated manifest metadata, generated catalog metadata, and generated artifact security checks.

`bash ci/local_check.sh` runs the smoke after tests so provider ontology behavior stays part of the normal local and CI loop.

Production-facing provider metadata must include explicit schema names and compatibility ranges when ontology support is advertised. Providers that do not support ontology behavior should omit the optional ontology metadata section.
