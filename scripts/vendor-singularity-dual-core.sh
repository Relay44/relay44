#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="${1:-${SOURCE_REPO_DIR:-}}"
DEST_DIR="$ROOT_DIR/vendor/singularity_dual_core"

if [[ -z "$SOURCE_DIR" ]]; then
  echo "Missing source path."
  echo "Set SOURCE_REPO_DIR or pass a path argument."
  echo "Example: SOURCE_REPO_DIR=../peer-core ./scripts/vendor-singularity-dual-core.sh"
  exit 1
fi

if [[ ! -d "$SOURCE_DIR" ]]; then
  echo "Source repo not found: $SOURCE_DIR"
  echo "Usage: ./scripts/vendor-singularity-dual-core.sh /path/to/source-repo"
  exit 1
fi

if [[ ! -d "$SOURCE_DIR/evm" ]]; then
  echo "Source appears invalid (missing evm/): $SOURCE_DIR"
  exit 1
fi

rm -rf "$DEST_DIR"
mkdir -p "$DEST_DIR"

mkdir -p "$DEST_DIR/evm"
rsync -a \
  --exclude 'out' \
  --exclude 'cache' \
  --exclude 'lib' \
  --exclude '.git' \
  --exclude 'foundry.lock' \
  "$SOURCE_DIR/evm/src" \
  "$SOURCE_DIR/evm/test" \
  "$SOURCE_DIR/evm/foundry.toml" \
  "$DEST_DIR/evm/"

mkdir -p "$DEST_DIR/docs"
for doc in \
  singularity-dual-native-domain-spec.md \
  singularity-dual-native-parity-matrix.md \
  singularity-legacy-migration-runbook.md; do
  if [[ -f "$SOURCE_DIR/docs/$doc" ]]; then
    cp "$SOURCE_DIR/docs/$doc" "$DEST_DIR/docs/$doc"
  fi
done

mkdir -p "$DEST_DIR/api-ts/src/services" "$DEST_DIR/api-ts/src/routes"
for file in \
  core-ids.ts \
  core-projections.ts \
  core-write-gate.ts; do
  if [[ -f "$SOURCE_DIR/services/singularity-api/src/services/$file" ]]; then
    cp "$SOURCE_DIR/services/singularity-api/src/services/$file" "$DEST_DIR/api-ts/src/services/$file"
  fi
done

for file in markets.ts orders.ts; do
  if [[ -f "$SOURCE_DIR/services/singularity-api/src/routes/$file" ]]; then
    cp "$SOURCE_DIR/services/singularity-api/src/routes/$file" "$DEST_DIR/api-ts/src/routes/$file"
  fi
done

cat > "$DEST_DIR/README.md" <<'EORD'
# Singularity Dual Core Vendor Snapshot

This directory is a one-way copy/paste snapshot imported into relay44 so relay44 can stay standalone.

Rules:
- No runtime imports from sibling repositories.
- This folder is reference/source material for local integration only.
- If re-vendoring, run `./scripts/vendor-singularity-dual-core.sh`.
EORD

stamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
cat > "$DEST_DIR/SNAPSHOT_META.txt" <<EOMETA
source_repo=$SOURCE_DIR
snapshot_utc=$stamp
EOMETA

echo "Vendored Singularity dual-core snapshot into $DEST_DIR"
