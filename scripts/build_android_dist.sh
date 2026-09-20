#!/usr/bin/env bash
# ==============================================================================
# Bolt Android 一键全量编译、打包与归档脚本 (JNI SO + APK)
# ==============================================================================
# 用法:
#   bash scripts/build_android_dist.sh [-m release|debug|all]
# ==============================================================================

set -euo pipefail

# 确保 Rust 工具链在 PATH 中
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck source=/dev/null
  . "$HOME/.cargo/env"
elif [ -d "$HOME/.cargo/bin" ]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ANDROID_APP_DIR="$ROOT_DIR/android_app"

MODE="release"
while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--mode)
      MODE="$2"
      shift 2
      ;;
    *)
      echo "未知参数: $1"
      echo "用法: $0 [-m release|debug|all]"
      exit 1
      ;;
  esac
done

echo "================================================="
echo "   Bolt Android Distribution Packaging Pipeline  "
echo "   Mode: $MODE                                   "
echo "================================================="

# 1. 自动定位 Android NDK
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  if [ -n "${ANDROID_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
    FOUND_NDK=$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
    if [ -n "$FOUND_NDK" ]; then
      export ANDROID_NDK_HOME="$FOUND_NDK"
    fi
  elif [ -d "$HOME/Android/Sdk/ndk" ]; then
    FOUND_NDK=$(find "$HOME/Android/Sdk/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
    if [ -n "$FOUND_NDK" ]; then
      export ANDROID_NDK_HOME="$FOUND_NDK"
    fi
  fi
fi

if [ -z "${ANDROID_NDK_HOME:-}" ] || [ ! -d "$ANDROID_NDK_HOME" ]; then
  echo "[错误] 未找到有效的 ANDROID_NDK_HOME，请确保已安装 Android NDK 并导出环境变量。"
  exit 1
fi
echo "[NDK] 使用 NDK 路径: $ANDROID_NDK_HOME"

# 2. 检查并编译 Rust JNI 动态库
ARM64_SO="$ANDROID_APP_DIR/app/src/main/jniLibs/arm64-v8a/libbt_ffi.so"
if [ ! -f "$ARM64_SO" ]; then
  echo ""
  echo "[1/3] 构建 Rust JNI 动态库 (cargo-ndk)..."
  if ! command -v cargo-ndk >/dev/null 2>&1; then
    echo "  正在安装 cargo-ndk..."
    cargo install cargo-ndk --locked
  fi

  cd "$ROOT_DIR"
  TARGETS=("aarch64-linux-android" "armv7-linux-androideabi" "x86_64-linux-android")
  for target in "${TARGETS[@]}"; do
    echo "  ---- 构建 $target ----"
    cargo ndk -t "$target" -o "$ANDROID_APP_DIR/app/src/main/jniLibs" build -p ffi --release
  done
  echo "  -> JNI 动态库构建成功: $ANDROID_APP_DIR/app/src/main/jniLibs"
else
  echo ""
  echo "[1/3] JNI 动态库已就绪，跳过重新编译。"
fi

# 3. 构建 APK
echo ""
echo "[2/3] 执行 Gradle 打包 APK ($MODE)..."
cd "$ANDROID_APP_DIR"
chmod +x ./gradlew

if [ "$MODE" = "all" ]; then
  ./gradlew assembleRelease assembleDebug
elif [ "$MODE" = "release" ]; then
  ./gradlew assembleRelease
else
  ./gradlew assembleDebug
fi

# 4. 产物汇总
echo ""
echo "================================================="
echo "            Android Packaging Summary            "
echo "================================================="
DIST_ANDROID="$ROOT_DIR/dist/android"
if [ -d "$DIST_ANDROID" ]; then
  find "$DIST_ANDROID" -name "*.apk" -exec ls -lh {} +
fi
echo "Android APK 构建完成！归档目录: $DIST_ANDROID"
