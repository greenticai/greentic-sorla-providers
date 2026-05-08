#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PACKS_DIR="examples/generated-packs"
CATALOG_FILE="examples/generated-catalog/provider-catalog.json"

OCI_REGISTRY="${OCI_REGISTRY:-ghcr.io}"
OCI_NAMESPACE="${OCI_NAMESPACE:-greenticai/sorla-providers}"
PROVIDER="${PROVIDER:-all}"
OCI_USERNAME="${OCI_USERNAME:?OCI_USERNAME is required}"
OCI_PASSWORD="${OCI_PASSWORD:?OCI_PASSWORD is required}"

PACK_ARTIFACT_TYPE="${PACK_ARTIFACT_TYPE:-application/vnd.greentic.sorla.provider-pack.v1+json}"
CATALOG_ARTIFACT_TYPE="${CATALOG_ARTIFACT_TYPE:-application/vnd.greentic.sorla.provider-catalog.v1+json}"

heading() {
  printf '\n==> %s\n' "$1"
}

provider_ref() {
  local slug="$1"
  local version="$2"
  printf '%s/%s/%s:%s' "$OCI_REGISTRY" "$OCI_NAMESPACE" "$slug" "$version"
}

cd "$ROOT_DIR"

rm -rf "$PACKS_DIR" "$(dirname "$CATALOG_FILE")"

heading "Generating provider packs"
cargo run -p sorla-provider-pack-cli -- --provider "$PROVIDER"

heading "Generating provider catalog"
cargo run -p sorla-provider-catalog-cli

heading "Logging into OCI registry"
printf '%s' "$OCI_PASSWORD" | oras login "$OCI_REGISTRY" --username "$OCI_USERNAME" --password-stdin

heading "Publishing provider pack artifacts"
find "$PACKS_DIR" -mindepth 2 -maxdepth 2 -name '*.gtpack.json' -print | sort | while IFS= read -r pack_file; do
  manifest_file=$(dirname "$pack_file")/manifest.json
  config_schema=$(dirname "$pack_file")/schemas/provider-config.schema.json
  provider_id=$(jq -r '.provider_id' "$manifest_file")
  provider_version=$(jq -r '.provider_version' "$manifest_file")
  provider_slug="${provider_id##*.}"
  provider_slug="${provider_slug#provider-}"
  ref=$(provider_ref "$provider_slug" "$provider_version")

  echo "Publishing ${provider_id} ${provider_version} -> oci://${ref}"
  oras push "$ref" \
    --artifact-type "$PACK_ARTIFACT_TYPE" \
    "${pack_file}:application/json" \
    "${manifest_file}:application/json" \
    "${config_schema}:application/schema+json"
done

heading "Publishing catalog artifact"
CATALOG_REF="$OCI_REGISTRY/$OCI_NAMESPACE/provider-catalog:$(date -u +%Y%m%d%H%M%S)"
echo "Publishing catalog -> ${CATALOG_REF}"
oras push "$CATALOG_REF" \
  --artifact-type "$CATALOG_ARTIFACT_TYPE" \
  "${CATALOG_FILE}:application/json" \
  "${PACKS_DIR}/index.json:application/json"
