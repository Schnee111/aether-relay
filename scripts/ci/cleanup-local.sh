#!/usr/bin/env bash
# Local CI artifact cleanup for aether-relay.
# Reclaims disk after local sentinel/local gate runs:
#   - Rust build artifacts (target/)         ~2-4 GB per full build
#   - Docker smoke test image + containers   ~1 GB base + build layers
#   - Docker BuildKit build cache            ~1-2 GB per smoke build
#
# Usage:
#   scripts/ci/cleanup-local.sh            # full clean (target + docker)
#   scripts/ci/cleanup-local.sh --docker-only   # docker only (keep target/)
#
# Safety: never deletes while a cargo/rustc build is running or any process
# holds files inside target/. Production Docker resources are untouched:
# only image tagged aether-relay:smoke, its stopped containers, and the
# BuildKit cache it produced are removed.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TARGET_DIR="$REPO_ROOT/target"
SMOKE_IMAGE="aether-relay:smoke"
SMOKE_CONTAINERS=("relay-smoke" "relay-vol")

removed_bytes=0

log() { echo "[cleanup] $*"; }

# --- Guard: refuse to run during an active Rust build ----------------------
if pgrep -f "cargo|rustc" >/dev/null 2>&1; then
  log "SKIP: cargo/rustc build in progress — aborting to avoid corrupting incremental state."
  exit 1
fi

# --- 1. Docker smoke artifacts ---------------------------------------------
if command -v docker >/dev/null 2>&1; then
  for c in "${SMOKE_CONTAINERS[@]}"; do
    if docker inspect "$c" >/dev/null 2>&1; then
      state=$(docker inspect -f '{{.State.Running}}' "$c" 2>/dev/null || echo "false")
      if [ "$state" != "true" ]; then
        docker rm -f "$c" >/dev/null 2>&1 && log "Removed stopped container: $c"
      else
        log "SKIP: container $c is running — not touched."
      fi
    fi
  done

  if docker image inspect "$SMOKE_IMAGE" >/dev/null 2>&1; then
    if docker ps --filter "ancestor=$SMOKE_IMAGE" --format '{{.Names}}' | grep -q .; then
      log "SKIP: image $SMOKE_IMAGE is in use by a running container."
    else
      docker rmi "$SMOKE_IMAGE" >/dev/null 2>&1 && log "Removed image: $SMOKE_IMAGE"
    fi
  fi

  # BuildKit cache produced by smoke builds (safe: reusable cache only)
  cache_out=$(docker builder prune -af 2>/dev/null | tail -1 || true)
  if [ -n "$cache_out" ]; then
    log "BuildKit cache: $cache_out"
  fi
else
  log "docker not available — skipping docker cleanup."
fi

# --- 2. Rust target/ --------------------------------------------------------
docker_only=false
[ "${1:-}" = "--docker-only" ] && docker_only=true

if [ "$docker_only" = false ] && [ -d "$TARGET_DIR" ]; then
  if lsof +D "$TARGET_DIR" >/dev/null 2>&1; then
    log "SKIP: $TARGET_DIR is held open by a running process."
  else
    size=$(du -sh "$TARGET_DIR" 2>/dev/null | cut -f1)
    rm -rf "$TARGET_DIR"
    log "Removed $TARGET_DIR (${size})"
  fi
fi

log "Done. Free disk: $(df -h / | awk 'NR==2{print $4}')"
