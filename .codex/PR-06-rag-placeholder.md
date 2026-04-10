# PR 06: Add RAG placeholder provider that generates deterministic fake evidence results

    **Repository:** `greenticai/greentic-sorla-providers`

    ## Objective

    Implement a RAG/evidence placeholder provider that returns deterministic fake retrieval results, citations, and provenance records suitable for testing the evidence flow and KAFD demo without requiring a real retrieval system yet.

    ## Why this PR exists

    The user also wants the RAG side to be a placeholder for now. The system still needs to show evidence lookup, source grounding, and attached citations. A deterministic fake retrieval provider lets us prove the architecture and demo flows immediately.

    ## Scope

    Build a provider that:
- accepts evidence/retrieval queries
- returns deterministic fake chunks/results based on the query and filters
- emits stable citation IDs, source refs, and snippet/provenance metadata
- can simulate retrieval from BTGs, RFIs, and site visits
- is packaged as a gtpack
Make it realistic enough for the KAFD demo to display evidence-backed answers.

    ## Deliverables

    - `sorla-provider-rag-mock` or similar crate
- deterministic fake retrieval engine
- query + metadata filter support
- mock citation/provenance structures
- provider config schema
- gtpack generation
- tests validating deterministic, stable outputs

    ## Implementation notes for Codex

    Make the fake retrieval plausible, not random noise. For example:
- if the building/floor query matches a known seeded fake source, prefer those fake chunks
- return page/section labels and snippet text with consistent formatting
- include a confidence-like signal if useful for the demo
Do not overfit to one demo question. The provider should feel generic enough for future replacement by a real retrieval system.

    ## Acceptance criteria

    - Same query and filters return the same fake evidence results
- Results include citations/provenance fields usable by downstream flows
- Pack generation works
- Tests prove determinism and contract compliance
- Docs clearly state mock status and future replacement path

    ## Non-goals

    - Real vector search
- Real embeddings/indexing
- Real document ingestion pipelines

    ## Suggested files / areas to touch

    - `crates/sorla-provider-rag-mock/`
- `docs/providers/rag-mock.md`
- tests for query determinism and fake evidence structure
