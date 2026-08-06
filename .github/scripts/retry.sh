#!/usr/bin/env bash
# Relance bornée pour opérations réseau transitoires uniquement.
# Usage: retry.sh <max_attempts> <command> [args...]
set -euo pipefail

max="${1:?max attempts required}"
shift

attempt=1
while true; do
  set +e
  "$@"
  status=$?
  set -e
  if [ "$status" -eq 0 ]; then
    exit 0
  fi
  if [ "$attempt" -ge "$max" ]; then
    echo "::error::Échec infrastructure après ${max} tentatives: $*" >&2
    exit "$status"
  fi
  sleep_for=$((attempt * 5))
  echo "::warning::Tentative ${attempt}/${max} échouée (exit ${status}); nouvel essai dans ${sleep_for}s…"
  sleep "$sleep_for"
  attempt=$((attempt + 1))
done
