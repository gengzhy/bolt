#!/usr/bin/env bash
# ============================================================
# Bolt 环回冒烟测试（M1/M2 验证门禁）
# 覆盖：配对 → 传输请求 → QUIC 传输 → BLAKE3 校验 → 内容一致
#      → 强制 TCP 传输（TOFU 信任复用，无二次配对）→ 断点续传
# 用法：bash scripts/smoke_loopback.sh [binary]
# ============================================================
set -u
cd "$(dirname "$0")/.."
BIN="${1:-./target/debug/bolt-cli.exe}"
PORT_A=8901
PORT_B=8902
ROOT=target/smoke
DATA_A="$ROOT/data_a"
DATA_B="$ROOT/data_b"

# 测试文件准备（8MB 随机 + 子目录 + 中文名 + 256MB 稀疏文件）
mkdir -p "$ROOT/src/sub" "$DATA_A" "$DATA_B"
[ -f "$ROOT/src/big.bin" ] || head -c 8388608 /dev/urandom > "$ROOT/src/big.bin"
printf 'hello bolt\n' > "$ROOT/src/sub/hello.txt"
printf '局部传输中文文件名测试\n' > "$ROOT/src/sub/文 件.txt"
truncate -s 256M "$ROOT/src/huge.bin" 2>/dev/null || head -c 268435456 /dev/zero > "$ROOT/src/huge.bin"

# 顺序执行各阶段；任一失败即退出非零
fail() { echo "SMOKE FAIL: $1"; exit 1; }

# 启动接收端（stdin 保持打开，自动应答配对/传输请求）
# 显式指定 save_dir：默认接收目录已改为系统下载目录，若依赖默认值
# 文件会落到用户真实 Downloads，脚本断言（data_a/out）恒失败
start_serve() {
    mkdir -p "$DATA_A"
    printf '{"save_dir":"%s/out"}\n' "$DATA_A" > "$DATA_A/config.json"
    ( printf 'y\ny\ny\ny\n'; sleep 300 ) \
        | RUST_LOG=info,transfer=debug,task=debug "$BIN" serve --port $PORT_A \
            --data-dir "$DATA_A" > "$ROOT/recv.log" 2>&1 &
    RECV_PID=$!
    sleep 5
}

# 终止接收端（cli 与持 stdin 的子壳一并结束；不 wait——子壳带 300s sleep）
stop_serve() {
    taskkill //IM bolt-cli.exe //F >/dev/null 2>&1
    kill "$RECV_PID" 2>/dev/null
    sleep 1
}

echo "== 阶段 1：QUIC 基础传输 =="
rm -rf "$DATA_A" "$DATA_B"
start_serve
timeout 60 env RUST_LOG=info "$BIN" send --port $PORT_B --data-dir "$DATA_B" \
    127.0.0.1:$PORT_A "$ROOT/src/big.bin" "$ROOT/src/sub" > "$ROOT/send.log" 2>&1 \
    || fail "阶段1 发送超时/失败"
grep -q "成功 3，失败 0" "$ROOT/send.log" || fail "阶段1 发送端汇总异常"
grep -q "成功 3，失败 0" "$ROOT/recv.log" || fail "阶段1 接收端汇总异常"
cmp "$ROOT/src/big.bin" "$DATA_A/out/big.bin" || fail "阶段1 big.bin 内容不一致"
cmp "$ROOT/src/sub/hello.txt" "$DATA_A/out/sub/hello.txt" || fail "阶段1 hello.txt 内容不一致"
cmp "$ROOT/src/sub/文 件.txt" "$DATA_A/out/sub/文 件.txt" || fail "阶段1 中文文件名内容不一致"
grep -q "connected quic" "$ROOT/recv.log" || fail "阶段1 未走 QUIC 通道"
echo "PASS 阶段1"

echo "== 阶段 2：强制 TCP（TOFU 信任复用，免二次配对）=="
stop_serve
rm -rf "$DATA_A/out"
start_serve
timeout 60 env BT_FORCE_TCP=1 RUST_LOG=info "$BIN" send --port $PORT_B --data-dir "$DATA_B" \
    127.0.0.1:$PORT_A "$ROOT/src/big.bin" > "$ROOT/send_tcp.log" 2>&1 \
    || fail "阶段2 发送超时/失败"
grep -q "成功 1，失败 0" "$ROOT/send_tcp.log" || fail "阶段2 发送端汇总异常"
grep -q "connected tcp" "$ROOT/recv.log" || fail "阶段2 未走 TCP 通道"
grep -q "请求配对" "$ROOT/recv.log" && fail "阶段2 不应出现二次配对"
cmp "$ROOT/src/big.bin" "$DATA_A/out/big.bin" || fail "阶段2 big.bin 内容不一致"
echo "PASS 阶段2"

echo "== 阶段 3：断点续传（256MB 中断后重连）=="
stop_serve
rm -rf "$DATA_A/out" "$DATA_A/tmp" "$DATA_A/resume"
start_serve
# 第一次发送：接收端临时文件达到 64MB 后杀死发送端（确定性中断，不依赖计时）
env RUST_LOG=info "$BIN" send --port $PORT_B --data-dir "$DATA_B" \
    127.0.0.1:$PORT_A "$ROOT/src/huge.bin" > "$ROOT/send_r1.log" 2>&1 &
SEND_PID=$!
for _ in $(seq 1 60); do
    size=$(stat -c %s "$DATA_A/tmp/task_1/f0.tmp" 2>/dev/null || echo 0)
    if [ "${size:-0}" -ge 67108864 ]; then break; fi
    sleep 0.25
done
taskkill //PID "$SEND_PID" //F >/dev/null 2>&1 || kill "$SEND_PID" 2>/dev/null
wait "$SEND_PID" 2>/dev/null
sleep 3 # 等接收端把断点缓存落盘（flush_acks 2s 节流）
ls "$DATA_A/resume"/*.json >/dev/null 2>&1 || fail "阶段3 未生成断点缓存"
# 第二次发送：续传完成
timeout 120 env RUST_LOG=info,transfer=debug "$BIN" send --port $PORT_B --data-dir "$DATA_B" \
    127.0.0.1:$PORT_A "$ROOT/src/huge.bin" > "$ROOT/send_r2.log" 2>&1 \
    || fail "阶段3 续传发送超时/失败"
grep -q "成功 1，失败 0" "$ROOT/send_r2.log" || fail "阶段3 续传发送端汇总异常"
grep -q "resume negotiated" "$ROOT/recv.log" || fail "阶段3 未触发断点协商"
cmp "$ROOT/src/huge.bin" "$DATA_A/out/huge.bin" || fail "阶段3 huge.bin 内容不一致"
echo "PASS 阶段3"

stop_serve
echo "SMOKE ALL PASS"
