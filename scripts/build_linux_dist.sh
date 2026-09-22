#!/usr/bin/env bash
# ==============================================================================
# Bolt Linux 一键全量编译、打包与归档脚本 (AppImage / DEB / RPM / CLI)
# ==============================================================================
# 用法:
#   bash scripts/build_linux_dist.sh [-m release|debug]
#
# 产物输出目录规范:
#   dist/linux/[debug|release]/
#   ├── appimage/ -> bolt-linux-{ver}-amd64.AppImage
#   ├── deb/      -> bolt-linux-{ver}-amd64.deb
#   ├── rpm/      -> bolt-linux-{ver}-1.x86_64.rpm
#   └── cli/      -> bolt-linux-{ver}-amd64-cli
# ==============================================================================

set -euo pipefail

# 确保 Rust 工具链在 PATH 中
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck source=/dev/null
  . "$HOME/.cargo/env"
elif [ -d "$HOME/.cargo/bin" ]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

# 脚本根目录定位
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# 默认构建模式
MODE="release"

while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--mode)
      MODE="$2"
      shift 2
      ;;
    *)
      echo "未知参数: $1"
      echo "用法: $0 [-m release|debug]"
      exit 1
      ;;
  esac
done

echo "=================================================================="
echo " Bolt Linux 全量编译打包流程启动 (模式: $MODE)"
echo "=================================================================="

# 1. 提取版本号
VERSION=$(grep -m1 '^version = ' "$ROOT_DIR/Cargo.toml" | sed -E 's/version = "(.*)"/\1/')
if [ -z "$VERSION" ]; then
  VERSION="0.1.0"
fi
echo "[1/5] 当前工程版本号: $VERSION"

# 2. 准备输出目录
DIST_LINUX_DIR="$ROOT_DIR/dist/linux/$MODE"
APPIMAGE_DIR="$DIST_LINUX_DIR/appimage"
DEB_DIR="$DIST_LINUX_DIR/deb"
RPM_DIR="$DIST_LINUX_DIR/rpm"
CLI_DIR="$DIST_LINUX_DIR/cli"

mkdir -p "$APPIMAGE_DIR" "$DEB_DIR" "$RPM_DIR" "$CLI_DIR"
echo "[2/5] 输出目录初始化完成: $DIST_LINUX_DIR"

CARGO_FLAGS=""
TAURI_FLAGS=""
TARGET_SUBDIR="release"

if [ "$MODE" = "debug" ]; then
  TARGET_SUBDIR="debug"
  TAURI_FLAGS="--debug"
else
  CARGO_FLAGS="--release"
fi

# 3. 编译命令行独立调试工具 (bolt-cli)
echo "[3/5] 编译控制台独立工具 bolt-cli..."
cd "$ROOT_DIR"
cargo build $CARGO_FLAGS -p bolt-cli

CLI_SRC="$ROOT_DIR/target/$TARGET_SUBDIR/bolt-cli"
CLI_TARGET="$CLI_DIR/bolt-linux-${VERSION}-amd64-cli"

if [ -f "$CLI_SRC" ]; then
  cp "$CLI_SRC" "$CLI_TARGET"
  chmod +x "$CLI_TARGET"
  if [ "$MODE" = "release" ] && command -v strip >/dev/null 2>&1; then
    strip "$CLI_TARGET" || true
  fi
  # 兼容旧命名别名
  cp "$CLI_TARGET" "$CLI_DIR/bolt_${VERSION}_amd64-cli"
  echo "  -> CLI 产物已生成: $CLI_TARGET"
else
  echo "  [警告] 未找到生成的 CLI 二进制: $CLI_SRC"
fi

# 4. 编译与打包 Tauri v2 桌面客户端
echo "[4/5] 构建 Tauri v2 Linux 桌面端与安装包..."
cd "$ROOT_DIR/tauri_app"

if [ ! -d "node_modules" ] || [ ! -d "node_modules/@tauri-apps/cli-linux-x64-gnu" ]; then
  echo "  补全 Linux 原生前端依赖与 CLI 二进制..."
  npm install --no-save @tauri-apps/cli-linux-x64-gnu 2>/dev/null || npm install
fi

echo "  执行 Tauri 打包 (targets: appimage, deb, rpm)..."
if [ -f "./node_modules/@tauri-apps/cli/tauri.js" ]; then
  node ./node_modules/@tauri-apps/cli/tauri.js build $TAURI_FLAGS --bundles appimage,deb,rpm || {
    echo "  [注意] 部分系统打包遇到局部警告，继续尝试收集可用产物..."
  }
else
  npx tauri build $TAURI_FLAGS --bundles appimage,deb,rpm || {
    echo "  [注意] 部分系统打包遇到局部警告，继续尝试收集可用产物..."
  }
fi

# 5. 归档与规范化重命名产物
echo "[5/5] 归档整理产物至 $DIST_LINUX_DIR..."

TAURI_BUNDLE_DIR="$ROOT_DIR/tauri_app/src-tauri/target/$TARGET_SUBDIR/bundle"

# 归档 AppImage
if [ -d "$TAURI_BUNDLE_DIR/appimage" ]; then
  FOUND_APPIMAGE=$(find "$TAURI_BUNDLE_DIR/appimage" -name "*.AppImage" | head -n 1)
  if [ -n "$FOUND_APPIMAGE" ] && [ -f "$FOUND_APPIMAGE" ]; then
    cp "$FOUND_APPIMAGE" "$APPIMAGE_DIR/bolt-linux-${VERSION}-amd64.AppImage"
    cp "$FOUND_APPIMAGE" "$APPIMAGE_DIR/bolt_${VERSION}_amd64.AppImage"
    chmod +x "$APPIMAGE_DIR/bolt-linux-${VERSION}-amd64.AppImage"
    echo "  -> AppImage 归档成功: $APPIMAGE_DIR/bolt-linux-${VERSION}-amd64.AppImage"
  fi
fi

# 归档 DEB
if [ -d "$TAURI_BUNDLE_DIR/deb" ]; then
  FOUND_DEB=$(find "$TAURI_BUNDLE_DIR/deb" -name "*.deb" | head -n 1)
  if [ -n "$FOUND_DEB" ] && [ -f "$FOUND_DEB" ]; then
    cp "$FOUND_DEB" "$DEB_DIR/bolt-linux-${VERSION}-amd64.deb"
    cp "$FOUND_DEB" "$DEB_DIR/bolt_${VERSION}_amd64.deb"
    echo "  -> DEB 归档成功: $DEB_DIR/bolt-linux-${VERSION}-amd64.deb"
  fi
fi

# 归档 RPM
if [ -d "$TAURI_BUNDLE_DIR/rpm" ]; then
  FOUND_RPM=$(find "$TAURI_BUNDLE_DIR/rpm" -name "*.rpm" | head -n 1)
  if [ -n "$FOUND_RPM" ] && [ -f "$FOUND_RPM" ]; then
    cp "$FOUND_RPM" "$RPM_DIR/bolt-linux-${VERSION}-1.x86_64.rpm"
    cp "$FOUND_RPM" "$RPM_DIR/bolt-${VERSION}-1.x86_64.rpm"
    echo "  -> RPM 归档成功: $RPM_DIR/bolt-linux-${VERSION}-1.x86_64.rpm"
  fi
fi

echo "=================================================================="
echo " Bolt Linux 全量打包完成！产物清单:"
echo "=================================================================="
ls -lh "$APPIMAGE_DIR" 2>/dev/null || true
ls -lh "$DEB_DIR" 2>/dev/null || true
ls -lh "$RPM_DIR" 2>/dev/null || true
ls -lh "$CLI_DIR" 2>/dev/null || true
echo "=================================================================="
