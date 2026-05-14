# Ontology Security

Generated provider pack and catalog artifacts must not contain secrets or credential-like metadata.

The provider-local ontology smoke checks generated JSON artifacts under:

- `examples/generated-packs`
- `examples/generated-catalog`

The check rejects common credential-like strings such as passwords, API keys, credentials, and private keys.

RAG mock evidence includes `permissions_context_json` so downstream flows can preserve sensitivity and authorization context alongside ontology-scoped evidence.

Provider ontology metadata should describe capabilities and compatibility only. Runtime credentials and startup answers do not belong in manifests, catalogs, or ontology capability metadata.
