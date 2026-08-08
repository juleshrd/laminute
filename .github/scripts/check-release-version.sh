#!/usr/bin/env bash
# Vérifie l'alignement des versions et que le commit taggé est sur main.
set -euo pipefail

PKG_VERSION="$(node -p "require('./package.json').version")"
TAURI_VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
CARGO_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' src-tauri/Cargo.toml | head -n1)"

if [[ -z "$PKG_VERSION" || -z "$TAURI_VERSION" || -z "$CARGO_VERSION" ]]; then
  echo "Impossible de lire les versions du projet." >&2
  exit 1
fi

if [[ "$PKG_VERSION" != "$TAURI_VERSION" || "$PKG_VERSION" != "$CARGO_VERSION" ]]; then
  echo "Versions désalignées :" >&2
  echo "  package.json          = $PKG_VERSION" >&2
  echo "  tauri.conf.json       = $TAURI_VERSION" >&2
  echo "  src-tauri/Cargo.toml  = $CARGO_VERSION" >&2
  exit 1
fi

echo "Versions alignées : $PKG_VERSION"

if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  TAG_NAME="${GITHUB_REF_NAME:?}"
  if [[ ! "$TAG_NAME" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
    echo "Tag invalide (attendu vX.Y.Z) : $TAG_NAME" >&2
    exit 1
  fi
  TAG_VERSION="${TAG_NAME#v}"
  if [[ "$TAG_VERSION" != "$PKG_VERSION" ]]; then
    echo "Le tag $TAG_NAME ne correspond pas à la version projet $PKG_VERSION." >&2
    exit 1
  fi

  git fetch --no-tags origin main
  if ! git merge-base --is-ancestor HEAD origin/main; then
    echo "Le commit taggé n'est pas un ancêtre de origin/main." >&2
    exit 1
  fi
  echo "Tag $TAG_NAME sur main OK."
fi
