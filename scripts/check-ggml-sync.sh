#!/usr/bin/env bash
#
# Verify that whisper.cpp and llama.cpp vendor a compatible copy of ggml.
#
# iOS is the only target that links both engines into one binary: desktop runs
# llama-helper as a subprocess, but iOS forbids subprocesses, so
# frontend/src-tauri depends on llama-helper directly there. whisper-rs-sys and
# llama-cpp-sys-2 each vendor their own ggml and export the same C symbols, so
# the linker keeps one copy of each `ggml_*` symbol and silently splices the two
# engines together.
#
# That splice is only safe while both copies are the same ggml. When they drift
# it is an ABI mismatch, not a link error: whisper-rs 0.13 against
# llama-cpp-sys-2 0.1.133 differed by one prepended field in
# `struct ggml_backend_reg`, so llama's register_backend() read whisper's iface
# one pointer too high and GGML_ASSERT aborted the app on every tap of record.
#
# Run this after bumping whisper-rs or llama-cpp-2. It needs both crates
# vendored, i.e. after at least one build (`cargo fetch` is enough).
#
# Exit status: 0 compatible, 1 drifted, 2 could not check.

set -uo pipefail

registry="${CARGO_HOME:-$HOME/.cargo}/registry/src"

# Newest vendored copy of each, by version sort.
latest_dir() {
    find "$registry" -maxdepth 2 \( -type d -o -type l \) -name "$1-*" 2>/dev/null | sort -V | tail -1
}

whisper_crate=$(latest_dir "whisper-rs-sys")
llama_crate=$(latest_dir "llama-cpp-sys-2")

if [ -z "$whisper_crate" ] || [ -z "$llama_crate" ]; then
    echo "check-ggml-sync: could not find both crates under $registry" >&2
    echo "  whisper-rs-sys: ${whisper_crate:-<missing>}" >&2
    echo "  llama-cpp-sys-2: ${llama_crate:-<missing>}" >&2
    echo "Run 'cargo fetch' in frontend/src-tauri first." >&2
    exit 2
fi

whisper_ggml="$whisper_crate/whisper.cpp/ggml"
llama_ggml="$llama_crate/llama.cpp/ggml"

for d in "$whisper_ggml" "$llama_ggml"; do
    if [ ! -d "$d" ]; then
        echo "check-ggml-sync: no vendored ggml at $d" >&2
        exit 2
    fi
done

echo "whisper.cpp ggml: $whisper_ggml"
echo "llama.cpp   ggml: $llama_ggml"
echo

# Only the parts that end up in an iOS binary. CUDA, Vulkan, SYCL and friends
# are never built for iOS, and whisper.cpp ships backends llama.cpp does not, so
# comparing the whole tree reports differences that cannot matter here.
paths=(
    "include"
    "src/ggml.c"
    "src/ggml-impl.h"
    "src/ggml-alloc.c"
    "src/ggml-backend.cpp"
    "src/ggml-backend-impl.h"
    "src/ggml-backend-reg.cpp"
    "src/ggml-quants.c"
    "src/ggml-quants.h"
    "src/ggml-common.h"
    "src/ggml-cpu"
    "src/ggml-metal"
)

status=0
missing=0
for path in "${paths[@]}"; do
    w="$whisper_ggml/$path"
    l="$llama_ggml/$path"

    if [ ! -e "$w" ] || [ ! -e "$l" ]; then
        echo "SKIP  $path (absent from one tree)"
        missing=1
        continue
    fi

    if diff -r -q "$w" "$l" >/dev/null 2>&1; then
        echo "ok    $path"
    else
        echo "DRIFT $path"
        diff -r -q "$w" "$l" 2>&1 \
            | sed -e "s|$whisper_ggml/|whisper.cpp:|g" \
                  -e "s|$llama_ggml/|llama.cpp:|g" \
                  -e 's/^/        /'
        status=1
    fi
done

echo
if [ "$status" -ne 0 ]; then
    cat >&2 <<'MSG'
ggml has drifted between whisper.cpp and llama.cpp.

Do not ship the iOS build like this: the two copies will be spliced by the
linker and the mismatch surfaces as an abort inside ggml, typically the first
time a WhisperContext is created.

Pick versions of whisper-rs and llama-cpp-2 whose vendored ggml matches (the
engines sync from the same upstream, so matching pairs do exist), or stop
linking both into one binary.
MSG
    exit 1
fi

if [ "$missing" -ne 0 ]; then
    echo "ggml looks compatible, but some paths were missing from one tree (see SKIP above)."
else
    echo "ggml is identical across whisper.cpp and llama.cpp. Safe to link both."
fi
