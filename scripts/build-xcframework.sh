#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CRATE_DIR="${REPO_ROOT}/rust/prism_core"
SWIFT_DIR="${REPO_ROOT}/swift/PrismFFI"
HEADERS_SRC_DIR="${SWIFT_DIR}/Sources/PrismFFI"
HEADERS_STAGE_DIR="$(mktemp -d "${SWIFT_DIR}/.xcframework-headers.XXXXXX")"
XCFRAMEWORK_NAME="PrismCoreFFI.xcframework"
XCFRAMEWORK_PATH="${SWIFT_DIR}/${XCFRAMEWORK_NAME}"
MANIFEST_PATH="${CRATE_DIR}/Cargo.toml"
TARGETS=(aarch64-apple-darwin)

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-13.0}"
export GIT2_STATIC=1
export LIBSSH2_SYS_STATIC=1
export OPENSSL_STATIC=1
export ZLIB_STATIC=1
export PKG_CONFIG_ALLOW_CROSS=1

cleanup() {
    rm -rf "${HEADERS_STAGE_DIR}"
}
trap cleanup EXIT

ensure_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: '$1' is required but was not found in PATH" >&2
        exit 1
    fi
}

ensure_command cargo
ensure_command xcodebuild

for target in "${TARGETS[@]}"; do
    if ! rustup target list --installed | grep -q "^${target}\$"; then
        echo "error: Rust target '${target}' is not installed. Run 'rustup target add ${target}'." >&2
        exit 1
    fi
done

echo "==> Building prism_core static libraries"
for target in "${TARGETS[@]}"; do
    echo "   -> ${target}"
    cargo build --manifest-path "${MANIFEST_PATH}" --release --target "${target}"
done

echo "==> Staging FFI headers"
mkdir -p "${HEADERS_STAGE_DIR}"
cp "${HEADERS_SRC_DIR}/prism_coreFFI.h" "${HEADERS_STAGE_DIR}/"
cp "${HEADERS_SRC_DIR}/prism_coreFFI.modulemap" "${HEADERS_STAGE_DIR}/module.modulemap"

LIB_AARCH64="${CRATE_DIR}/target/aarch64-apple-darwin/release/libprism_core.a"

if [[ ! -f "${LIB_AARCH64}" ]]; then
    echo "error: expected static library was not produced" >&2
    exit 1
fi

echo "==> Creating ${XCFRAMEWORK_NAME}"
rm -rf "${XCFRAMEWORK_PATH}"
xcodebuild -create-xcframework \
    -library "${LIB_AARCH64}" -headers "${HEADERS_STAGE_DIR}" \
    -output "${XCFRAMEWORK_PATH}"

echo "✅  Wrote ${XCFRAMEWORK_PATH}"
