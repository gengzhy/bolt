<script setup lang="ts">
// 传输记录日志弹窗面板（轻量拟物风，独立全览视图）。
// 展示内容：发送/接收、协议名称 (QUIC/TCP)、文件名称、文件大小、传输速率、传输开始/结束时间、耗时、状态、对端名称。

import { computed, ref, onMounted, onUnmounted } from "vue";
import { useTransferLogs } from "../composables/useTransferLogs";
import { human, humanRate } from "../utils/format";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "update:open", v: boolean): void }>();

const { logs, clearLogs } = useTransferLogs();

const filter = ref<"all" | "send" | "recv">("all");
const confirmClear = ref(false);

const filteredLogs = computed(() => {
  if (filter.value === "send") return logs.value.filter((x) => x.direction === "send");
  if (filter.value === "recv") return logs.value.filter((x) => x.direction === "recv");
  return logs.value;
});

const sendCount = computed(() => logs.value.filter((x) => x.direction === "send").length);
const recvCount = computed(() => logs.value.filter((x) => x.direction === "recv").length);

function close() {
  emit("update:open", false);
  confirmClear.value = false;
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && props.open) {
    close();
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleKeydown);
});
onUnmounted(() => {
  window.removeEventListener("keydown", handleKeydown);
});

function formatDate(ms: number): string {
  if (!ms || ms <= 0) return "—";
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function formatDuration(ms: number): string {
  if (!ms || ms <= 0) return "—";
  if (ms < 1000) return `${ms} ms`;
  const secs = (ms / 1000).toFixed(1);
  return `${secs} s`;
}

function stateText(s: string): string {
  switch (s) {
    case "done": return "完成";
    case "error": return "失败";
    case "cancelled": return "已取消";
    case "transferring": return "传输中";
    case "waiting_accept": return "等待接受";
    default: return s || "完成";
  }
}

function stateClass(s: string): string {
  if (s === "done") return "ok";
  if (s === "error") return "err";
  if (s === "cancelled") return "warn";
  return "run";
}

function executeClear() {
  clearLogs();
  confirmClear.value = false;
}
</script>

<template>
  <div v-if="open" class="log-overlay" @click.self="close">
    <div class="log-dialog">
      <!-- 头部：标题、统计、关闭 -->
      <div class="dialog-head">
        <div class="head-title">
          <div class="title-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
              <line x1="16" y1="13" x2="8" y2="13" />
              <line x1="16" y1="17" x2="8" y2="17" />
              <polyline points="10 9 9 9 8 9" />
            </svg>
          </div>
          <div>
            <h2>传输记录日志</h2>
            <span class="sub-text">共 {{ logs.length }} 条传输记录</span>
          </div>
        </div>

        <div class="head-actions">
          <button v-if="logs.length > 0 && !confirmClear" class="btn-clear" @click="confirmClear = true">
            清空日志
          </button>
          <div v-if="confirmClear" class="clear-confirm-pop">
            <span>确定清空所有日志？</span>
            <button class="confirm-yes" @click="executeClear">确定</button>
            <button class="confirm-no" @click="confirmClear = false">取消</button>
          </div>
          <button class="close-btn" title="关闭" @click="close">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      </div>

      <!-- 过滤标签栏 -->
      <div class="filter-bar">
        <div class="filter-chips">
          <button class="chip" :class="{ active: filter === 'all' }" @click="filter = 'all'">
            全部 ({{ logs.length }})
          </button>
          <button class="chip" :class="{ active: filter === 'send' }" @click="filter = 'send'">
            发送 ({{ sendCount }})
          </button>
          <button class="chip" :class="{ active: filter === 'recv' }" @click="filter = 'recv'">
            接收 ({{ recvCount }})
          </button>
        </div>
      </div>

      <!-- 日志列表主内容区 -->
      <div class="dialog-body">
        <div v-if="filteredLogs.length === 0" class="empty-state">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="empty-icon">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
            <polyline points="14 2 14 8 20 8" />
          </svg>
          <p class="empty-title">暂无传输记录日志</p>
          <p class="empty-desc">发起或接收文件后，详细的传输审计日志将自动记录在此处。</p>
        </div>

        <div v-else class="log-cards">
          <div v-for="item in filteredLogs" :key="item.id" class="log-card">
            <!-- 卡片第一行：方向、协议、对端、状态 -->
            <div class="card-row-1">
              <span class="dir-badge" :class="item.direction">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                  <path v-if="item.direction === 'send'" d="M12 19V5M5 12l7-7 7 7" />
                  <path v-else d="M12 5v14M19 12l-7 7-7-7" />
                </svg>
                {{ item.direction === "send" ? "发送" : "接收" }}
              </span>

              <span v-if="item.transport" class="proto-badge">
                {{ item.transport.toUpperCase() }}
              </span>

              <span class="peer-name" :title="item.peer_name">
                · {{ item.peer_name || "未知设备" }}
              </span>

              <span class="spacer"></span>

              <span class="state-badge" :class="stateClass(item.state)">
                {{ stateText(item.state) }}
              </span>
            </div>

            <!-- 卡片第二行：文件名与包含项数 -->
            <div class="card-row-2">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="file-icon">
                <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
                <polyline points="13 2 13 9 20 9" />
              </svg>
              <span class="file-name" :title="item.file_name">{{ item.file_name }}</span>
            </div>

            <!-- 卡片第三行：关键指标网格（大小、速率、耗时） -->
            <div class="metrics-grid">
              <div class="metric-item">
                <span class="metric-label">文件大小</span>
                <span class="metric-val">{{ human(item.total_size) }}</span>
              </div>
              <div class="metric-item">
                <span class="metric-label">传输速率</span>
                <span class="metric-val accent">{{ humanRate(item.avg_rate_bps) }}</span>
              </div>
              <div class="metric-item">
                <span class="metric-label">传输耗时</span>
                <span class="metric-val">{{ formatDuration(item.duration_ms) }}</span>
              </div>
            </div>

            <!-- 卡片第四行：时间轴展示 -->
            <div class="timeline-row">
              <div class="time-item">
                <span class="time-dot start"></span>
                <span>开始：{{ formatDate(item.start_time_ms) }}</span>
              </div>
              <div class="time-item">
                <span class="time-dot end"></span>
                <span>结束：{{ item.end_time_ms > 0 ? formatDate(item.end_time_ms) : (item.state === 'transferring' ? '传输中...' : '—') }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.log-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  animation: fadeIn 0.2s ease-out;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.log-dialog {
  width: 90%;
  max-width: 720px;
  height: 82vh;
  max-height: 760px;
  background: var(--panel, #ffffff);
  border: 1px solid var(--line, rgba(0, 0, 0, 0.1));
  border-radius: var(--r-modal, 18px);
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.25);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: scaleUp 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes scaleUp {
  from { transform: scale(0.96); opacity: 0.7; }
  to { transform: scale(1); opacity: 1; }
}

.dialog-head {
  padding: 16px 20px;
  border-bottom: 1px solid var(--line);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.head-title {
  display: flex;
  align-items: center;
  gap: 12px;
}

.title-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  background: var(--accent-soft);
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
}

.head-title h2 {
  font-size: 16.5px;
  font-weight: 700;
  margin: 0;
  color: var(--text);
}

.sub-text {
  font-size: 11.5px;
  color: var(--muted);
}

.head-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  position: relative;
}

.btn-clear {
  background: none;
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 5px 10px;
  font-size: 12.5px;
  color: var(--muted);
  cursor: pointer;
  transition: all 0.15s;
}
.btn-clear:hover {
  background: rgba(239, 68, 68, 0.08);
  color: var(--danger, #ef4444);
  border-color: rgba(239, 68, 68, 0.3);
}

.clear-confirm-pop {
  position: absolute;
  right: 40px;
  top: 0;
  background: var(--panel);
  border: 1px solid var(--line);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.15);
  border-radius: 8px;
  padding: 6px 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  z-index: 10;
  white-space: nowrap;
}
.confirm-yes {
  background: var(--danger, #ef4444);
  color: #ffffff;
  border: none;
  border-radius: 6px;
  padding: 3px 8px;
  cursor: pointer;
}
.confirm-no {
  background: none;
  border: 1px solid var(--line);
  color: var(--muted);
  border-radius: 6px;
  padding: 3px 8px;
  cursor: pointer;
}

.close-btn {
  background: none;
  border: none;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--muted);
  cursor: pointer;
}
.close-btn:hover {
  background: var(--line);
  color: var(--text);
}

.filter-bar {
  padding: 10px 20px;
  border-bottom: 1px solid var(--line);
  display: flex;
  align-items: center;
}

.filter-chips {
  display: flex;
  gap: 8px;
}

.chip {
  background: none;
  border: 1px solid var(--line);
  border-radius: 20px;
  padding: 4px 12px;
  font-size: 12px;
  color: var(--muted);
  cursor: pointer;
  transition: all 0.15s;
}
.chip:hover {
  border-color: var(--accent);
  color: var(--accent);
}
.chip.active {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--accent);
  font-weight: 600;
}

.dialog-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 16px 20px;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 0;
  color: var(--muted);
  text-align: center;
}
.empty-icon {
  margin-bottom: 12px;
  opacity: 0.5;
}
.empty-title {
  font-size: 14.5px;
  font-weight: 600;
  margin: 0 0 6px;
  color: var(--text);
}
.empty-desc {
  font-size: 12.5px;
  max-width: 280px;
  margin: 0;
  line-height: 1.5;
}

.log-cards {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.log-card {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 12px;
  padding: 12px 14px;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.04);
  display: flex;
  flex-direction: column;
  gap: 8px;
  transition: transform 0.15s, box-shadow 0.15s;
}
.log-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
}

.card-row-1 {
  display: flex;
  align-items: center;
  gap: 8px;
}

.dir-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  font-weight: 700;
  padding: 2px 7px;
  border-radius: 6px;
}
.dir-badge.send {
  background: var(--accent-soft);
  color: var(--accent);
}
.dir-badge.recv {
  background: rgba(16, 185, 129, 0.12);
  color: var(--ok, #10b981);
}

.proto-badge {
  font-size: 10px;
  font-weight: 800;
  padding: 2px 6px;
  border-radius: 4px;
  background: rgba(139, 92, 246, 0.12);
  color: #8b5cf6;
  letter-spacing: 0.5px;
}

.peer-name {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 220px;
}

.spacer {
  flex: 1;
}

.state-badge {
  font-size: 11.5px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 6px;
}
.state-badge.ok {
  background: rgba(16, 185, 129, 0.12);
  color: var(--ok, #10b981);
}
.state-badge.err {
  background: rgba(239, 68, 68, 0.12);
  color: var(--danger, #ef4444);
}
.state-badge.warn {
  background: rgba(245, 158, 11, 0.12);
  color: var(--warn, #f59e0b);
}
.state-badge.run {
  background: var(--accent-soft);
  color: var(--accent);
}

.card-row-2 {
  display: flex;
  align-items: center;
  gap: 8px;
}
.file-icon {
  color: var(--accent);
  flex-shrink: 0;
}
.file-name {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.metrics-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 8px 12px;
  gap: 8px;
}
.metric-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.metric-label {
  font-size: 11px;
  color: var(--muted);
}
.metric-val {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text);
}
.metric-val.accent {
  color: var(--accent);
}

.timeline-row {
  display: flex;
  justify-content: space-between;
  font-size: 11.5px;
  color: var(--muted);
  padding: 2px 2px 0;
}
.time-item {
  display: flex;
  align-items: center;
  gap: 6px;
}
.time-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}
.time-dot.start {
  background: var(--accent);
}
.time-dot.end {
  background: var(--ok, #10b981);
}
</style>
