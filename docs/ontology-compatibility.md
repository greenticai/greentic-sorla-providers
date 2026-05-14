# Ontology Compatibility

Ontology-aware providers advertise compatibility inside manifest `ontology_capabilities.compatibility`.

The compatibility block includes:

- `supported_ontology_schema`
- `supported_ontology_schema_range`
- optional `supported_retrieval_binding_schema`
- optional `supported_external_mapping_schema`

Schema ranges are parsed with semver requirements. Unknown or malformed ranges are rejected by tests and smoke checks.

Generated catalog entries preserve the same compatibility values in their optional `ontology` section, allowing discovery tools to select compatible providers without loading provider code.
