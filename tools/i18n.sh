#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-all}"
AUTH_MODE="${AUTH_MODE:-auto}"
LOCALE="${LOCALE:-en}"
EN_PATH="${EN_PATH:-i18n/en.json}"
LOCALES_PATH="${LOCALES_PATH:-i18n/locales.json}"
BATCH_SIZE="${I18N_BATCH_SIZE:-200}"

auth_mode_supported() {
  if [[ "$AUTH_MODE" == "auto" || "$AUTH_MODE" == "api-key" || "$AUTH_MODE" == "browser" ]]; then
    return 0
  fi
  return 1
}

usage() {
  cat <<'EOF_USAGE'
Usage: tools/i18n.sh [translate|validate|status|all]

Environment overrides:
  EN_PATH=...                      English source file path (default: i18n/en.json)
  LOCALES_PATH=...                 Locale list file path (default: i18n/locales.json)
  AUTH_MODE=auto|api-key|browser    translator auth mode (default: auto)
  LOCALE=en                        CLI locale used for translator output (default: en)
  I18N_BATCH_SIZE=<int>            target translations per batch (default: 200)

Modes:
  translate  Translate en.json to all supported locales in 200-item batches.
  validate   Validate translation files against i18n/en.json
  status     Show translation status for all locales
  all        Run translate + validate + status

EOF_USAGE
}

log() {
  printf '[i18n] %s\n' "$*"
}

fail() {
  printf '[i18n] error: %s\n' "$*" >&2
  exit 1
}

require_tool() {
  local tool="$1"
  command -v "$tool" >/dev/null 2>&1 || fail "required command not found: ${tool}"
}

ensure_translator() {
  if command -v greentic-i18n-translator >/dev/null 2>&1; then
    return
  fi

  command -v cargo-binstall >/dev/null 2>&1 \
    || fail "greentic-i18n-translator not found and cargo-binstall is unavailable"

  log "installing greentic-i18n-translator via cargo-binstall"
  cargo binstall -y greentic-i18n-translator \
    || fail "failed to install greentic-i18n-translator via cargo-binstall"

  command -v greentic-i18n-translator >/dev/null 2>&1 \
    || fail "greentic-i18n-translator is still not on PATH after cargo-binstall"
}

load_locales() {
  require_tool jq
  if [[ ! -f "$LOCALES_PATH" ]]; then
    fail "missing locales file: ${LOCALES_PATH}"
  fi
  jq -r '.[]' "$LOCALES_PATH"
}

ensure_locale_files() {
  for lang in "${LOCALE_LIST[@]}"; do
    local locale_file="i18n/${lang}.json"
    if [[ ! -f "$locale_file" ]]; then
      printf '{\n}\n' > "$locale_file"
      log "created locale file: ${locale_file}"
    fi
  done
}

locale_csv() {
  local langs=("$@")
  local IFS=','
  printf '%s\n' "${langs[*]}"
}

split_translate_batch() {
  local start="$1"
  local size="$2"
  local source_file="$3"
  local batch_file="$4"

  jq -s --argjson start "$start" --argjson size "$size" '
    .[0]
    | to_entries
    | .[$start:($start + $size)]
    | from_entries
  ' "$source_file" > "$batch_file"
}

run_translate_batch() {
  local en_file="$1"
  local langs="$2"
  local supports_batch_size support

  if command -v rg >/dev/null 2>&1; then
    support="$(greentic-i18n-translator --help 2>/dev/null | rg -o -- "--batch-size" || true)"
  else
    support="$(greentic-i18n-translator --help 2>/dev/null | grep -Eo -- "--batch-size" || true)"
  fi

  if [[ -n "$support" ]]; then
    greentic-i18n-translator \
      --locale "$LOCALE" \
      translate --langs "$langs" --en "$en_file" --batch-size "$BATCH_SIZE" --auth-mode "$AUTH_MODE"
  else
    greentic-i18n-translator \
      --locale "$LOCALE" \
      translate --langs "$langs" --en "$en_file" --auth-mode "$AUTH_MODE"
  fi
}

run_translate() {
  require_tool jq

  if [[ ! -f "$EN_PATH" ]]; then
    fail "missing English source file: ${EN_PATH}"
  fi

  mapfile -t LOCALE_LIST < <(load_locales)
  if [[ ${#LOCALE_LIST[@]} -eq 0 ]]; then
    fail "no locales found in ${LOCALES_PATH}"
  fi

  ensure_locale_files

  if ! auth_mode_supported; then
    fail "unsupported AUTH_MODE '${AUTH_MODE}'. expected auto|api-key|browser"
  fi

  local keys
  keys=$(jq 'length' "$EN_PATH")

  if (( keys <= 0 )); then
    log "nothing to translate in ${EN_PATH}"
    return
  fi

  local locales_csv="$(locale_csv "${LOCALE_LIST[@]}")"
  local batch_size="${BATCH_SIZE}"
  if ! [[ "$batch_size" =~ ^[1-9][0-9]*$ ]]; then
    batch_size=200
  fi

  if (( keys <= batch_size )); then
    run_translate_batch "$EN_PATH" "$locales_csv"
    return
  fi

  local start=0
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' RETURN

  while (( start < keys )); do
    local batch_file="$tmp_dir/en.${start}.json"
    split_translate_batch "$start" "$batch_size" "$EN_PATH" "$batch_file"
    run_translate_batch "$batch_file" "$locales_csv"
    start=$(( start + batch_size ))
  done
}

run_validate() {
  require_tool jq

  mapfile -t LOCALE_LIST < <(load_locales)
  if [[ ${#LOCALE_LIST[@]} -eq 0 ]]; then
    fail "no locales found in ${LOCALES_PATH}"
  fi

  local locales_csv="$(locale_csv "${LOCALE_LIST[@]}")"
  greentic-i18n-translator \
    --locale "$LOCALE" \
    validate --langs "$locales_csv" --en "$EN_PATH"
}

run_status() {
  require_tool jq

  mapfile -t LOCALE_LIST < <(load_locales)
  if [[ ${#LOCALE_LIST[@]} -eq 0 ]]; then
    fail "no locales found in ${LOCALES_PATH}"
  fi

  local locales_csv="$(locale_csv "${LOCALE_LIST[@]}")"
  greentic-i18n-translator \
    --locale "$LOCALE" \
    status --langs "$locales_csv" --en "$EN_PATH"
}

if [[ "${MODE}" == "-h" || "${MODE}" == "--help" ]]; then
  usage
  exit 0
fi

ensure_translator

case "$MODE" in
  translate)
    run_translate
    ;;
  validate)
    run_validate
    ;;
  status)
    run_status
    ;;
  all)
    run_translate
    run_validate
    run_status
    ;;
  *)
    echo "Unknown mode: $MODE" >&2
    usage
    exit 2
    ;;
esac
