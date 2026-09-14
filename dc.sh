#!/usr/bin/env bash
# Local build / multi-arch Hub push via compose files.
#
# Usage:
#   ./dc.sh build              # docker compose -f build.yml build
#   ./dc.sh push               # docker compose -f push.yml build --no-cache --push
#   ./dc.sh build [args...]    # extra args passed through to compose build
#   ./dc.sh push [args...]

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

usage() {
  cat <<'EOF'
Usage:
  ./dc.sh build              # docker compose -f build.yml build
  ./dc.sh push               # docker compose -f push.yml build --no-cache --push
  ./dc.sh build [args...]    # extra args passed through to compose build
  ./dc.sh push [args...]
EOF
  exit "${1:-0}"
}

cmd="${1:-}"
[[ -n "$cmd" ]] || usage 1
shift || true

case "$cmd" in
  build)
    exec docker compose -f build.yml build "$@"
    ;;
  push)
    exec docker compose -f push.yml build --no-cache --push "$@"
    ;;
  -h|--help|help)
    usage 0
    ;;
  *)
    echo "Unknown command: $cmd" >&2
    usage 1
    ;;
esac
