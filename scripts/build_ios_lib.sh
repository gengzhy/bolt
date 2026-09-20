#!/usr/bin/env bash
# ============================================================
# 构建 iOS 端 Rust FFI 静态库与 XCFramework (BoltEngine.xcframework)
#
# 前置依赖：
# 1. macOS 环境与 Xcode Command Line Tools
# 2. Rust iOS 目标工具链：
#    rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
# ============================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

MODE="${1:-release}"
CARGO_FLAG=""
if [ "${MODE}" = "release" ]; then
    CARGO_FLAG="--release"
fi

echo "============================================================"
echo "Bolt iOS 核心引擎编译 (Mode: ${MODE})"
echo "============================================================"

# 1. 确保目标平台就绪
echo "[1/4] 检查并安装 Rust iOS 编译目标..."
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios >/dev/null 2>&1 || true

# 2. 编译各平台静态库 (libbt_ffi.a)
echo "[2/4] 编译 Rust 静态库..."
echo "  -> aarch64-apple-ios (真机 arm64)"
cargo build -p ffi --target aarch64-apple-ios ${CARGO_FLAG}

echo "  -> aarch64-apple-ios-sim (Apple Silicon 模拟器)"
cargo build -p ffi --target aarch64-apple-ios-sim ${CARGO_FLAG}

echo "  -> x86_64-apple-ios (Intel Mac 模拟器)"
cargo build -p ffi --target x86_64-apple-ios ${CARGO_FLAG}

# 3. 组织产物与模拟器通用库
TARGET_DIR="${ROOT_DIR}/target"
OUTPUT_DIR="${ROOT_DIR}/ios_app/Frameworks"
HEADERS_DIR="${ROOT_DIR}/crates/ffi/include"
SIM_UNIVERSAL_DIR="${TARGET_DIR}/ios-sim-universal/${MODE}"

mkdir -p "${SIM_UNIVERSAL_DIR}"
mkdir -p "${OUTPUT_DIR}"

echo "[3/4] 合并模拟器架构 (lipo: aarch64-sim + x86_64-sim)..."
lipo -create \
    "${TARGET_DIR}/aarch64-apple-ios-sim/${MODE}/libbt_ffi.a" \
    "${TARGET_DIR}/x86_64-apple-ios/${MODE}/libbt_ffi.a" \
    -output "${SIM_UNIVERSAL_DIR}/libbt_ffi.a"

# 4. 生成 XCFramework
echo "[4/4] 打包 BoltEngine.xcframework..."
rm -rf "${OUTPUT_DIR}/BoltEngine.xcframework"

xcodebuild -create-xcframework \
    -library "${TARGET_DIR}/aarch64-apple-ios/${MODE}/libbt_ffi.a" \
    -headers "${HEADERS_DIR}" \
    -library "${SIM_UNIVERSAL_DIR}/libbt_ffi.a" \
    -headers "${HEADERS_DIR}" \
    -output "${OUTPUT_DIR}/BoltEngine.xcframework"

# 同时拷贝头文件到 ios_app/Bolt/FFI/ 便于 Xcode 快速索引与代码补全
mkdir -p "${ROOT_DIR}/ios_app/Bolt/FFI"
cp -f "${HEADERS_DIR}/bt_api.h" "${ROOT_DIR}/ios_app/Bolt/FFI/bt_api.h"

echo "============================================================"
echo "[SUCCESS] iOS 静态库与 XCFramework 构建完成！"
echo "产物路径: ${OUTPUT_DIR}/BoltEngine.xcframework"
echo "头文件已同步至: ${ROOT_DIR}/ios_app/Bolt/FFI/bt_api.h"
echo "============================================================"
