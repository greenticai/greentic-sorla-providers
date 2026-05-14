# Ontology Smoke Scenario

This repo includes a provider-local ontology smoke command:

```bash
cargo xtask ontology-smoke
```

The command is deterministic and does not require network services or a real FoundationDB cluster.

It verifies:

- FoundationDB local/dev generic entity and relationship storage
- deterministic path finding from `Customer` to `EvidenceDocument`
- SharePoint mock external mapping validation
- SharePoint mock deterministic entity linking
- RAG mock ontology-scoped evidence with linked entities and permissions context
- provider manifest ontology metadata
- provider catalog ontology compatibility metadata

The command prints a stable JSON summary with counts for graph paths, SharePoint links, evidence items, and catalog entries.
