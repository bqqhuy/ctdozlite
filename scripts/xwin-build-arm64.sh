#!/usr/bin/env bash
# Cross-build the Windows ARM64 binary from Linux with cargo-xwin (dev check only).
#
# Usage: scripts/xwin-build-arm64.sh [build|check|clippy] [cargo args...]
#   e.g. scripts/xwin-build-arm64.sh build --bin ctdozlite
#        scripts/xwin-build-arm64.sh clippy --all-targets -- -D warnings
# Requires: rustup target aarch64-pc-windows-msvc, cargo-xwin, clang/lld/llvm, cmake.
# cargo-xwin downloads the MSVC CRT and Windows SDK into ~/.cache/cargo-xwin; running this
# script means accepting the Microsoft license terms for them (XWIN_ACCEPT_LICENSE=1).
# The release package is still built and tested natively by build-windows-arm64.ps1 / CI.
set -euo pipefail
clang_bin=$(command -v clang)
cmake_bin=$(command -v cmake)
# A fixed folder: cargo-xwin caches a clang-cl symlink to the first `clang` on PATH.
tools="${XDG_CACHE_HOME:-$HOME/.cache}/ctdozlite-xwin/bin"
mkdir -p "$tools"
# ring compiles its ARM64 assembly with GNU-style clang, which rejects cl-style /imsvc.
# Called as clang-cl (through cargo-xwin's symlink) it must stay in cl mode, untouched.
cat > "$tools/clang" <<WRAP
#!/usr/bin/env bash
if [ "\$(basename "\$0")" = clang-cl ]; then exec -a clang-cl "$clang_bin" "\$@"; fi
args=()
for a in "\$@"; do
  if [ "\$a" = "/imsvc" ]; then args+=("-isystem"); else args+=("\$a"); fi
done
exec "$clang_bin" "\${args[@]}"
WRAP
# CMake defaults to /MD; the binary links the CRT statically (+crt-static), so use /MT.
cat > "$tools/cmake" <<WRAP
#!/usr/bin/env bash
case " \$* " in
  *" --build "*|*" -E "*|*" --version "*) exec "$cmake_bin" "\$@" ;;
esac
exec "$cmake_bin" "\$@" \\
  "-DCMAKE_C_FLAGS_RELEASE=/MT /O2 /Ob2 /DNDEBUG" \\
  "-DCMAKE_CXX_FLAGS_RELEASE=/MT /O2 /Ob2 /DNDEBUG" \\
  "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded"
WRAP
chmod +x "$tools/clang" "$tools/cmake"
# Re-point a stale cargo-xwin symlink left by an older wrapper location.
link="${XDG_CACHE_HOME:-$HOME/.cache}/cargo-xwin/clang-cl"
if [ -L "$link" ] && [ "$(readlink "$link")" != "$tools/clang" ]; then rm "$link"; fi
export PATH="$tools:$PATH" CMAKE="$tools/cmake" XWIN_ACCEPT_LICENSE=1 XWIN_ARCH=aarch64 \
  CMAKE_POLICY_VERSION_MINIMUM=3.5
command=${1:-build}
[ $# -gt 0 ] && shift
cargo xwin "$command" --release --locked --target aarch64-pc-windows-msvc "$@"
