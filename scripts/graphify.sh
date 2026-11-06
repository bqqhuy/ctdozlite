#!/usr/bin/env bash
# Keep graph generation local and independent of the Windows application build.
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$repo_root"
graphify_bin="${GRAPHIFY_BIN:-graphify}"
if ! command -v "$graphify_bin" >/dev/null 2>&1; then
    echo 'Graphify chưa được cài. Xem docs/GRAPHIFY.md.' >&2
    exit 127
fi
"$graphify_bin" extract . --code-only --cargo --max-workers 2 --out graphify-out/ast "$@"
graphify_python="${GRAPHIFY_PYTHON:-$(dirname -- "$(command -v "$graphify_bin")")/python}"
if [[ ! -x "$graphify_python" ]]; then graphify_python=python3; fi
"$graphify_python" scripts/graphify_roadmap.py --ast-graph graphify-out/ast/graphify-out/graph.json
"$graphify_bin" cluster-only . --no-label
"$graphify_bin" tree --label ctdozlite
