#!/usr/bin/env bash
# ==============================================================================
# Bolt iOS 一键全量编译、打包与归档脚本 (IPA / Archive)
# ==============================================================================
# 用法:
#   bash scripts/build_ios_dist.sh [-m release|debug]
#
# 产物输出目录规范:
#   dist/ios/[debug|release]/
#   ├── bolt_{ver}_ios[-debug].ipa  -> 免越狱侧载直接可装包
#   ├── bolt.ipa / Bolt.ipa         -> 兼容别名
#   └── Bolt.xcarchive              -> Xcode 标准归档包
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
IOS_DIR="${ROOT_DIR}/ios_app"

# 默认构建参数
MODE="release"

while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--mode)
      MODE="$2"
      shift 2
      ;;
    release|Release)
      MODE="release"
      shift 1
      ;;
    debug|Debug)
      MODE="debug"
      shift 1
      ;;
    *)
      echo "未知参数: $1"
      echo "用法: $0 [-m release|debug]"
      exit 1
      ;;
  esac
done

MODE_LOWER="$(echo "$MODE" | tr '[:upper:]' '[:lower:]')"
if [ "$MODE_LOWER" = "debug" ]; then
  CONFIGURATION="Debug"
  IPA_SUFFIX="-debug"
else
  CONFIGURATION="Release"
  IPA_SUFFIX=""
fi

# 1. 提取版本号 (与 Windows / Android / macOS 端对齐)
VERSION=$(grep -m1 '^version = ' "$ROOT_DIR/Cargo.toml" | sed -E 's/version = "(.*)"/\1/')
if [ -z "$VERSION" ]; then
  VERSION="0.1.0"
fi

# 2. 统一输出目录：dist/ios/[release|debug]/
DIST_DIR="${ROOT_DIR}/dist/ios/${MODE_LOWER}"
mkdir -p "${DIST_DIR}"

echo "============================================================"
echo "Bolt iOS 应用构建与打包"
echo "模式: ${MODE_LOWER} (Xcode Configuration: ${CONFIGURATION}) | 版本: ${VERSION}"
echo "============================================================"

# 3. 确保底层核心 XCFramework 库就绪
if [ ! -d "${IOS_DIR}/Frameworks/BoltEngine.xcframework" ]; then
  echo "[1/3] 检测到 BoltEngine.xcframework 未就绪，正在编译..."
  bash "${SCRIPT_DIR}/build_ios_lib.sh" "${MODE_LOWER}"
fi

# 4. 执行 xcodebuild archive
ARCHIVE_PATH="${DIST_DIR}/Bolt.xcarchive"
rm -rf "${ARCHIVE_PATH}"

echo "[2/3] 正在执行 Xcode Archive 构建..."
xcodebuild archive \
    -project "${IOS_DIR}/Bolt.xcodeproj" \
    -scheme "Bolt" \
    -configuration "${CONFIGURATION}" \
    -destination "generic/platform=iOS" \
    -archivePath "${ARCHIVE_PATH}" \
    CODE_SIGNING_ALLOWED=NO

# 5. 自动生成无签名发行版 IPA（便于通过 Sideloadly / AltStore / TrollStore / 爱思助手等直接侧载安装）
echo "[3/3] 正在打包标准 IPA 安装包..."
PAYLOAD_DIR="${DIST_DIR}/Payload"
rm -rf "${PAYLOAD_DIR}"
mkdir -p "${PAYLOAD_DIR}"

IPA_NAME="bolt_${VERSION}_ios${IPA_SUFFIX}.ipa"
IPA_PATH="${DIST_DIR}/${IPA_NAME}"
rm -f "${IPA_PATH}"

if [ -d "${ARCHIVE_PATH}/Products/Applications/Bolt.app" ]; then
    cp -r "${ARCHIVE_PATH}/Products/Applications/Bolt.app" "${PAYLOAD_DIR}/"
    (cd "${DIST_DIR}" && zip -qr "${IPA_NAME}" "Payload")
    rm -rf "${PAYLOAD_DIR}"
    
    # 建立同目录下兼容别名
    cp "${IPA_PATH}" "${DIST_DIR}/bolt.ipa"
    cp "${IPA_PATH}" "${DIST_DIR}/Bolt.ipa"
    # 向前兼容旧版根 dist/ios/ 路径
    mkdir -p "${ROOT_DIR}/dist/ios"
    cp "${IPA_PATH}" "${ROOT_DIR}/dist/ios/Bolt.ipa" 2>/dev/null || true
    
    echo "  -> IPA 打包完成: ${IPA_PATH}"
else
    echo "[!] 警告: 未在 Archive 中找到 Bolt.app，跳过 IPA 打包。"
fi

echo "============================================================"
echo "Bolt iOS 打包完成！产物归档于: ${DIST_DIR}"
echo "  - 标准 IPA 安装包: ${IPA_PATH}"
echo "  - 兼容格式别名:   ${DIST_DIR}/Bolt.ipa"
echo "  - Xcode 归档:      ${ARCHIVE_PATH}"
echo "============================================================"
