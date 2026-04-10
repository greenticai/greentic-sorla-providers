# Provider Packs

PR-03 introduces deterministic local provider pack generation for the SoRLa provider family.

## Goals

- generate stable local pack artifacts from shared provider metadata
- keep one canonical manifest shape across FoundationDB, SharePoint mock, and RAG mock
- make later OCI/GHCR publication a follow-on step rather than the source of truth

## Local Output Layout

Run:

```bash
cargo run -p sorla-provider-pack-cli
```

By default this writes to `examples/generated-packs/`.

Each provider gets its own deterministic directory:

- `manifest.json`
  Canonical provider pack manifest with capabilities, compatibility, runtime component refs, config schema ref, status, and display metadata.

- `*.gtpack.json`
  Local serialized gtpack artifact envelope. This is the current local pack artifact produced by the workspace.

- `schemas/provider-config.schema.json`
  Stub configuration schema shipped with the generated artifact.

The generator also writes `examples/generated-packs/index.json` listing every generated provider pack.

## Manifest Shape

The canonical manifest currently includes:

- provider ID
- provider version
- provider kind
- capability list
- mock or real flag
- provider status
- supported provider contract version
- supported SoRLa IR and IR range
- artifact references
- runtime component references
- configuration schema reference
- optional OCI reference
- display metadata

## OCI and GHCR

OCI and GHCR publishing are now layered on top of local deterministic generation rather than replacing it.

The release path is:

1. generate deterministic local pack artifacts
2. generate the catalog artifact from those packs
3. publish crates.io crates
4. publish the generated pack and catalog JSON outputs to GHCR as OCI artifacts
5. attach the same generated outputs to the GitHub Release

The canonical source of truth remains the generated local manifest and pack layout.

### Published OCI References

For a tag `vX.Y.Z`, the release workflow publishes:

- `ghcr.io/<owner>/<repo>/<provider-id>-pack:vX.Y.Z`
- `ghcr.io/<owner>/<repo>/provider-catalog:vX.Y.Z`

Each provider pack OCI artifact includes:

- `*.gtpack.json`
- `manifest.json`
- `schemas/provider-config.schema.json`

The catalog OCI artifact includes:

- `examples/generated-catalog/provider-catalog.json`
- `examples/generated-packs/index.json`

The workflow uses OCI artifact media types rather than container images so the pack and catalog JSON stay aligned with the local deterministic outputs.
