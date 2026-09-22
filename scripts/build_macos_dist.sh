#!/usr/bin/env bash
# ==============================================================================
# Bolt macOS 一键全量编译、打包与归档脚本 (DMG / APP / CLI)
# ==============================================================================
# 用法:
#   bash scripts/build_macos_dist.sh [-m release|debug] [-t target]
#
# 示例:
#   bash scripts/build_macos_dist.sh -m release
#   bash scripts/build_macos_dist.sh -m release -t aarch64-apple-darwin
#   bash scripts/build_macos_dist.sh -m release -t universal-apple-darwin
#
# 产物输出目录规范:
#   dist/macos/[debug|release]/
#   ├── dmg/ -> bolt-macos-{ver}-{arch}.dmg
#   ├── app/ -> Bolt.app & bolt-macos-{ver}-{arch}.app.zip
#   └── cli/ -> bolt-macos-{ver}-{arch}-cli
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

# 默认构建参数
MODE="release"
CUSTOM_TARGET=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--mode)
      MODE="$2"
      shift 2
      ;;
    -t|--target)
      CUSTOM_TARGET="$2"
      shift 2
      ;;
    *)
      echo "未知参数: $1"
      echo "用法: $0 [-m release|debug] [-t target]"
      exit 1
      ;;
  esac
done

# 检测架构
HOST_ARCH="$(uname -m)"
if [ "$HOST_ARCH" = "arm64" ] || [ "$HOST_ARCH" = "aarch64" ]; then
  ARCH_LABEL="aarch64"
elif [ "$HOST_ARCH" = "x86_64" ]; then
  ARCH_LABEL="x64"
else
  ARCH_LABEL="$HOST_ARCH"
fi

if [ -n "$CUSTOM_TARGET" ]; then
  if [[ "$CUSTOM_TARGET" =~ "universal" ]]; then
    ARCH_LABEL="universal"
  elif [[ "$CUSTOM_TARGET" =~ "aarch64" ]]; then
    ARCH_LABEL="aarch64"
  elif [[ "$CUSTOM_TARGET" =~ "x86_64" ]]; then
    ARCH_LABEL="x64"
  fi
fi

echo "=================================================================="
echo " Bolt macOS 全量编译打包流程启动"
echo " 模式: $MODE | 目标架构: $ARCH_LABEL | 自定义Target: ${CUSTOM_TARGET:-'(默认本机)'}"
echo "=================================================================="

# 1. 提取版本号
VERSION=$(grep -m1 '^version = ' "$ROOT_DIR/Cargo.toml" | sed -E 's/version = "(.*)"/\1/')
if [ -z "$VERSION" ]; then
  VERSION="0.1.0"
fi
echo "[1/5] 当前工程版本号: $VERSION"

# 2. 准备输出目录
DIST_MACOS_DIR="$ROOT_DIR/dist/macos/$MODE"
DMG_DIR="$DIST_MACOS_DIR/dmg"
APP_DIR="$DIST_MACOS_DIR/app"
CLI_DIR="$DIST_MACOS_DIR/cli"

mkdir -p "$DMG_DIR" "$APP_DIR" "$CLI_DIR"
echo "[2/5] 输出目录初始化完成: $DIST_MACOS_DIR"

CARGO_FLAGS=""
TAURI_FLAGS=""
TARGET_SUBDIR="release"

if [ "$MODE" = "debug" ]; then
  TARGET_SUBDIR="debug"
  TAURI_FLAGS="--debug"
else
  CARGO_FLAGS="--release"
fi

if [ -n "$CUSTOM_TARGET" ]; then
  TAURI_FLAGS="$TAURI_FLAGS --target $CUSTOM_TARGET"
fi

# 3. 编译命令行独立调试工具 (bolt-cli)
echo "[3/5] 编译控制台独立工具 bolt-cli..."
cd "$ROOT_DIR"

if [ "$CUSTOM_TARGET" = "universal-apple-darwin" ]; then
  echo "  构建 universal 架构 bolt-cli (aarch64 + x86_64)..."
  # shellcheck disable=SC2086
  cargo build $CARGO_FLAGS -p bolt-cli --target aarch64-apple-darwin
  # shellcheck disable=SC2086
  cargo build $CARGO_FLAGS -p bolt-cli --target x86_64-apple-darwin
  mkdir -p "$ROOT_DIR/target/universal-apple-darwin/$TARGET_SUBDIR"
  lipo -create \
    "$ROOT_DIR/target/aarch64-apple-darwin/$TARGET_SUBDIR/bolt-cli" \
    "$ROOT_DIR/target/x86_64-apple-darwin/$TARGET_SUBDIR/bolt-cli" \
    -output "$ROOT_DIR/target/universal-apple-darwin/$TARGET_SUBDIR/bolt-cli"
  CLI_SRC="$ROOT_DIR/target/universal-apple-darwin/$TARGET_SUBDIR/bolt-cli"
elif [ -n "$CUSTOM_TARGET" ]; then
  # shellcheck disable=SC2086
  cargo build $CARGO_FLAGS -p bolt-cli --target "$CUSTOM_TARGET"
  CLI_SRC="$ROOT_DIR/target/$CUSTOM_TARGET/$TARGET_SUBDIR/bolt-cli"
else
  # shellcheck disable=SC2086
  cargo build $CARGO_FLAGS -p bolt-cli
  CLI_SRC="$ROOT_DIR/target/$TARGET_SUBDIR/bolt-cli"
fi

CLI_TARGET="$CLI_DIR/bolt-macos-${VERSION}-${ARCH_LABEL}-cli"

if [ -f "$CLI_SRC" ]; then
  cp "$CLI_SRC" "$CLI_TARGET"
  chmod +x "$CLI_TARGET"
  if [ "$MODE" = "release" ] && command -v strip >/dev/null 2>&1; then
    strip "$CLI_TARGET" || true
  fi
  # 兼容旧命名别名
  cp "$CLI_TARGET" "$CLI_DIR/bolt_${VERSION}_${ARCH_LABEL}-cli"
  echo "  -> CLI 产物已生成: $CLI_TARGET"
else
  echo "  [警告] 未找到生成的 CLI 二进制: $CLI_SRC"
fi

# 4. 编译与打包 Tauri v2 macOS 桌面客户端
echo "[4/5] 构建 Tauri v2 macOS 桌面端 (DMG / APP)..."
cd "$ROOT_DIR/tauri_app"

if [ ! -d "node_modules" ]; then
  echo "  安装前端依赖..."
  npm install
fi

echo "  执行 Tauri 打包 (targets: dmg, app)..."
# shellcheck disable=SC2086
npx tauri build $TAURI_FLAGS --bundles dmg,app || {
  echo "  [注意] 部分系统打包遇到局部警告，继续尝试收集可用产物..."
}

# 5. 归档与规范化产物
echo "[5/5] 归档整理产物至 $DIST_MACOS_DIR..."

if [ -n "$CUSTOM_TARGET" ]; then
  TAURI_BUNDLE_DIR="$ROOT_DIR/tauri_app/src-tauri/target/$CUSTOM_TARGET/$TARGET_SUBDIR/bundle"
else
  TAURI_BUNDLE_DIR="$ROOT_DIR/tauri_app/src-tauri/target/$TARGET_SUBDIR/bundle"
fi

# 归档 APP Bundle
FOUND_APP=""
for app_candidate in \
  "$TAURI_BUNDLE_DIR/macos" "$TAURI_BUNDLE_DIR/osx" "$TAURI_BUNDLE_DIR/app" \
  "$ROOT_DIR/tauri_app/src-tauri/target/$TARGET_SUBDIR/bundle/macos" \
  "$ROOT_DIR/tauri_app/src-tauri/target/$TARGET_SUBDIR/bundle/osx"; do
  if [ -d "$app_candidate" ]; then
    MATCH=$(find "$app_candidate" -maxdepth 1 -name "*.app" | head -n 1)
    if [ -n "$MATCH" ] && [ -d "$MATCH" ]; then
      FOUND_APP="$MATCH"
      break
    fi
  fi
done

if [ -n "$FOUND_APP" ] && [ -d "$FOUND_APP" ]; then
  cp -R "$FOUND_APP" "$APP_DIR/"
  APP_BASENAME="$(basename "$FOUND_APP")"
  echo "  -> APP Bundle 归档成功: $APP_DIR/$APP_BASENAME"

  # 打包压缩 zip 便于分发与 CI 上传
  ZIP_TARGET="$APP_DIR/bolt-macos-${VERSION}-${ARCH_LABEL}.app.zip"
  (cd "$APP_DIR" && zip -r -q "$ZIP_TARGET" "$APP_BASENAME")
  # 兼容旧命名别名
  cp "$ZIP_TARGET" "$APP_DIR/bolt_${VERSION}_${ARCH_LABEL}.app.zip"
  echo "  -> APP Zip 压缩包已生成: $ZIP_TARGET"
fi

# 归档 DMG
FOUND_DMG=""
for dmg_dir in \
  "$TAURI_BUNDLE_DIR/dmg" \
  "$ROOT_DIR/tauri_app/src-tauri/target/$TARGET_SUBDIR/bundle/dmg" \
  "$TAURI_BUNDLE_DIR/macos" \
  "$TAURI_BUNDLE_DIR/osx"; do
  if [ -d "$dmg_dir" ]; then
    MATCH=$(find "$dmg_dir" -name "*.dmg" ! -name "rw.*" 2>/dev/null | head -n 1)
    if [ -n "$MATCH" ] && [ -f "$MATCH" ]; then
      FOUND_DMG="$MATCH"
      break
    fi
  fi
done

DMG_TARGET="$DMG_DIR/bolt-macos-${VERSION}-${ARCH_LABEL}.dmg"
if [ -n "$FOUND_DMG" ] && [ -f "$FOUND_DMG" ]; then
  cp "$FOUND_DMG" "$DMG_TARGET"
  cp "$DMG_TARGET" "$DMG_DIR/bolt_${VERSION}_${ARCH_LABEL}.dmg"
  echo "  -> DMG 归档成功: $DMG_TARGET"
elif [ -n "$FOUND_APP" ] && [ -d "$FOUND_APP" ]; then
  echo "  [提示] 未发现标准 Tauri DMG，使用 macOS 原生 hdiutil 构建 DMG 镜像..."
  rm -f "$DMG_TARGET"
  hdiutil create -volname "Bolt" -srcfolder "$FOUND_APP" -ov -format UDZO "$DMG_TARGET" || {
    echo "  [警告] hdiutil 打包 DMG 失败。"
  }
  if [ -f "$DMG_TARGET" ]; then
    cp "$DMG_TARGET" "$DMG_DIR/bolt_${VERSION}_${ARCH_LABEL}.dmg"
    echo "  -> DMG (hdiutil) 归档成功: $DMG_TARGET"
  fi
else
  echo "  [警告] 未找到生成的 DMG 镜像或 APP Bundle。"
fi

echo "=================================================================="
echo " Bolt macOS 全量打包完成！产物清单:"
echo "=================================================================="
ls -lh "$DMG_DIR" 2>/dev/null || true
ls -lh "$APP_DIR" 2>/dev/null || true
ls -lh "$CLI_DIR" 2>/dev/null || true
echo "=================================================================="
