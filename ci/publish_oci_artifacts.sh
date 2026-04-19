#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PACKS_DIR="examples/generated-packs"
CATALOG_FILE="examples/generated-catalog/provider-catalog.json"

OCI_REGISTRY="${OCI_REGISTRY:-ghcr.io}"
OCI_NAMESPACE="${OCI_NAMESPACE:?OCI_NAMESPACE is required}"
OCI_TAG="${OCI_TAG:?OCI_TAG is required}"
OCI_USERNAME="${OCI_USERNAME:?OCI_USERNAME is required}"
OCI_PASSWORD="${OCI_PASSWORD:?OCI_PASSWORD is required}"

PACK_ARTIFACT_TYPE="${PACK_ARTIFACT_TYPE:-application/vnd.greentic.sorla.provider-pack.v1+json}"
CATALOG_ARTIFACT_TYPE="${CATALOG_ARTIFACT_TYPE:-application/vnd.greentic.sorla.provider-catalog.v1+json}"

heading() {
  printf '\n==> %s\n' "$1"
}

artifact_ref() {
  local name="$1"
  printf '%s/%s/%s:%s' "$OCI_REGISTRY" "$OCI_NAMESPACE" "$name" "$OCI_TAG"
}

cd "$ROOT_DIR"

heading "Generating provider packs"
cargo run -p sorla-provider-pack-cli

heading "Generating provider catalog"
cargo run -p sorla-provider-catalog-cli

heading "Logging into OCI registry"
printf '%s' "$OCI_PASSWORD" | oras login "$OCI_REGISTRY" --username "$OCI_USERNAME" --password-stdin

heading "Publishing provider pack artifacts"
find "$PACKS_DIR" -mindepth 2 -maxdepth 2 -name '*.gtpack.json' -print | sort | while IFS= read -r pack_file; do
  pack_name=$(basename "$pack_file" .gtpack.json)
  manifest_file=$(dirname "$pack_file")/manifest.json
  config_schema=$(dirname "$pack_file")/schemas/provider-config.schema.json
  ref=$(artifact_ref "${pack_name}-pack")

  echo "Publishing ${pack_name} -> ${ref}"
  oras push "$ref" \
    --artifact-type "$PACK_ARTIFACT_TYPE" \
    "${pack_file}:application/json" \
    "${manifest_file}:application/json" \
    "${config_schema}:application/schema+json"
done

heading "Publishing catalog artifact"
CATALOG_REF=$(artifact_ref "provider-catalog")
echo "Publishing catalog -> ${CATALOG_REF}"
oras push "$CATALOG_REF" \
  --artifact-type "$CATALOG_ARTIFACT_TYPE" \
  "${CATALOG_FILE}:application/json" \
  "${PACKS_DIR}/index.json:application/json"
