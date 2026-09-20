#!/usr/bin/env bash
# ============================================================
# 构建 iOS 原生发行包 (IPA / Archive)
#
# 前置依赖：
# 1. 已运行 bash scripts/build_ios_lib.sh 生成 BoltEngine.xcframework
# 2. 已配置 Apple Developer 签名证书或使用非签名导出
# ============================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
IOS_DIR="${ROOT_DIR}/ios_app"
DIST_DIR="${ROOT_DIR}/dist/ios"

CONFIGURATION="${1:-Release}"

echo "============================================================"
echo "Bolt iOS 应用构建与打包 (Configuration: ${CONFIGURATION})"
echo "============================================================"

# 1. 确保核心库就绪
if [ ! -d "${IOS_DIR}/Frameworks/BoltEngine.xcframework" ]; then
    echo "[!] 未检测到 BoltEngine.xcframework，正在触发底层库编译..."
    bash "${SCRIPT_DIR}/build_ios_lib.sh" "${CONFIGURATION,,}"
fi

# 2. 清理与创建产物目录
mkdir -p "${DIST_DIR}"
ARCHIVE_PATH="${DIST_DIR}/Bolt.xcarchive"
rm -rf "${ARCHIVE_PATH}"

# 3. 执行 xcodebuild archive
echo "[1/2] 正在执行 Xcode Archive 构建..."
xcodebuild archive \
    -project "${IOS_DIR}/Bolt.xcodeproj" \
    -scheme "Bolt" \
    -configuration "${CONFIGURATION}" \
    -destination "generic/platform=iOS" \
    -archivePath "${ARCHIVE_PATH}" \
    CODE_SIGNING_ALLOWED=NO \
    LIBRARY_SEARCH_PATHS="\$(inherited) ${IOS_DIR}/Frameworks/BoltEngine.xcframework/ios-arm64 ${ROOT_DIR}/target/aarch64-apple-ios/${CONFIGURATION,,}" \
    OTHER_LDFLAGS="\$(inherited) -framework Security -framework Network -framework SystemConfiguration -lbt_ffi"

# 4. 自动生成无签名发行版 IPA（便于通过 Sideloadly / AltStore / TrollStore 等在 Windows 上直接侧载安装）
echo "[2/3] 正在打包 Bolt.ipa..."
PAYLOAD_DIR="${DIST_DIR}/Payload"
rm -rf "${PAYLOAD_DIR}" "${DIST_DIR}/Bolt.ipa"
mkdir -p "${PAYLOAD_DIR}"

if [ -d "${ARCHIVE_PATH}/Products/Applications/Bolt.app" ]; then
    cp -r "${ARCHIVE_PATH}/Products/Applications/Bolt.app" "${PAYLOAD_DIR}/"
    (cd "${DIST_DIR}" && zip -qr "Bolt.ipa" "Payload")
    rm -rf "${PAYLOAD_DIR}"
    echo "[3/3] Bolt.ipa 打包完成！"
else
    echo "[!] 警告: 未在 Archive 中找到 Bolt.app，跳过 IPA 打包。"
fi

echo "============================================================"
echo "产物归档于: ${DIST_DIR}"
echo "  - IPA 安装包: ${DIST_DIR}/Bolt.ipa"
echo "  - Xcode 归档: ${ARCHIVE_PATH}"
echo "============================================================"
