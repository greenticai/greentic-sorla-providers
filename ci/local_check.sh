#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

print_step() {
    echo
    echo "=================================================="
    echo "$1"
    echo "=================================================="
}

list_publishable_crates() {
    if command -v python3 >/dev/null 2>&1; then
        local metadata_json
        metadata_json=$(mktemp)
        cargo metadata --no-deps --format-version 1 >"$metadata_json"
        python3 - "$metadata_json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as fh:
    data = json.load(fh)
for pkg in data.get("packages", []):
    publish = pkg.get("publish", None)
    if publish is False:
        continue
    if isinstance(publish, list) and len(publish) == 0:
        continue
    print(f"{pkg['name']}|{pkg['manifest_path']}")
PY
        rm -f "$metadata_json"
        return
    fi

    if grep -q '^name[[:space:]]*=' Cargo.toml; then
        local fallback_name
        fallback_name=$(awk -F ' = ' '/^name[[:space:]]*=/{gsub(/"/, "", $2); print $2; exit}' Cargo.toml)
        echo "${fallback_name}|${REPO_ROOT}/Cargo.toml"
        return
    fi
}

PACKAGE_MODE="full"
if [[ "${1:-}" == "--package-only" ]]; then
    PACKAGE_MODE="package-only"
elif [[ "${1:-}" == "--list-publishable" ]]; then
    PACKAGE_MODE="list-publishable"
fi

if [[ "$PACKAGE_MODE" == "list-publishable" ]]; then
    list_publishable_crates
    exit 0
fi

run_package_checks=0
if [[ "$PACKAGE_MODE" == "package-only" || "$PACKAGE_MODE" == "full" ]]; then
    run_package_checks=1
fi

run_full=0
if [[ "$PACKAGE_MODE" == "full" ]]; then
    run_full=1
fi

if [[ "$run_full" -eq 1 ]]; then
    print_step "cargo fmt"
    cargo fmt --all -- --check

    print_step "cargo clippy"
    cargo clippy --all-targets --all-features -- -D warnings

    print_step "cargo test"
    cargo test --all-features

    print_step "cargo build"
    cargo build --all-features

    print_step "cargo doc"
    cargo doc --no-deps --all-features
fi

if [[ "$run_package_checks" -eq 1 ]]; then
    print_step "Packaging checks"
    tmp_publish_list=$(mktemp)
    trap 'rm -f "$tmp_publish_list"' EXIT

    list_publishable_crates >"$tmp_publish_list"

    if [[ ! -s "$tmp_publish_list" ]]; then
        echo "No publishable crates found in cargo metadata."
        exit 1
    fi

    while IFS='|' read -r crate manifest_path; do
        if [[ -z "$crate" || -z "$manifest_path" ]]; then
            continue
        fi

        printf '\n--- %s ---\n' "$crate"

        echo "package --no-verify"
        cargo package --no-verify --allow-dirty --manifest-path "$manifest_path"

        echo "package --allow-dirty"
        cargo package --allow-dirty --manifest-path "$manifest_path"

        echo "publish --dry-run"
        if [[ "${CI:-}" == "true" ]]; then
            cargo publish --dry-run --manifest-path "$manifest_path"
        else
            cargo publish --allow-dirty --dry-run --manifest-path "$manifest_path"
        fi

        package_listing=$(mktemp)
        cargo package --allow-dirty --manifest-path "$manifest_path" --list >"$package_listing"

        if ! grep -Eq '(^|/)src/' "$package_listing"; then
            echo "Publish guardrail failed for $crate: source files missing from package contents"
            exit 1
        fi
        if ! grep -Eq '(^|/)Cargo.toml$' "$package_listing"; then
            echo "Publish guardrail failed for $crate: Cargo.toml missing from package contents"
            exit 1
        fi
        if ! grep -Eq '(^|/)README(\.md)?$' "$package_listing"; then
            echo "Publish guardrail failed for $crate: README missing from package contents"
            exit 1
        fi
        if ! grep -Eq '(^|/)LICENSE(\.md)?$' "$package_listing"; then
            echo "Publish guardrail failed for $crate: LICENSE missing from package contents"
            exit 1
        fi
        rm -f "$package_listing"
    done <"$tmp_publish_list"

    echo
    echo "Packaging checks completed."
fi
