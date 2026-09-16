<script setup lang="ts">
// Bolt 桌面端主界面（轻量拟物风 #4，按设计文档实现）。
// 结构：自定义标题栏（拖拽区 + 标题居中 + 设置/最小化/最大化/关闭）
//       三栏内容区（左 280 设备 / 中 自适应 发送 / 右 300 任务）
//       底部 32px 全局状态栏。

import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useBt } from "./composables/useBt";
import DevicePanel from "./components/DevicePanel.vue";
import SendPanel from "./components/SendPanel.vue";
import TaskPanel from "./components/TaskPanel.vue";
import SettingsModal from "./components/SettingsModal.vue";
import Modals from "./components/Modals.vue";
import logoSvg from "./assets/logo.svg";

const { version, toast, devices, connStates, progress, localInfo, localIpsText, start, stop } = useBt();
const settingsOpen = ref(false);
const win = getCurrentWindow();
const maximized = ref(false);

async function syncMaximized() {
  maximized.value = await win.isMaximized();
}
function minimize() {
  void win.minimize();
}
async function toggleMax() {
  await win.toggleMaximize();
  await syncMaximized();
}
function closeWin() {
  void win.close();
}

// 状态栏：当前协议（取任一已连接会话的通道）
const protocol = computed(() => {
  const list = Object.values(connStates);
  if (list.length === 0) return "未连接";
  return list.some((c) => (c.transport ?? "").toLowerCase().includes("quic"))
    ? "QUIC"
    : (list[0]?.transport ?? "TCP").toUpperCase();
});
// 状态栏：实时速度 = 所有进行中任务速率之和
const totalRate = computed(() =>
  Object.values(progress).reduce((s, p) => s + (p.rate_bps || 0), 0),
);
function fmtRate(bps: number): string {
  if (bps <= 0) return "0 MB/s";
  return `${(bps / 1024 / 1024).toFixed(1)} MB/s`;
}

// ---------- 两栏宽度调节与移动端紧凑自适应 ----------
const COMPACT_BREAKPOINT = 680;
const LEFT_MIN = 300;
const RIGHT_MIN = 300;
const leftW = ref(370);
const isCompact = ref(typeof window !== "undefined" ? window.innerWidth < COMPACT_BREAKPOINT : false);

function handleResize() {
  isCompact.value = window.innerWidth < COMPACT_BREAKPOINT;
  if (!isCompact.value) {
    leftW.value = clampLeft(leftW.value);
  }
}

let maxTimer: number | undefined;
onMounted(() => {
  start();
  void syncMaximized();
  handleResize();
  window.addEventListener("resize", handleResize);
  maxTimer = window.setInterval(() => void syncMaximized(), 2000);
});
onUnmounted(() => {
  stop();
  window.removeEventListener("resize", handleResize);
  window.clearInterval(maxTimer);
});

function clampLeft(v: number): number {
  // 窗口内宽 - 左右外边距(24) - 拖拽槽(12) - 右侧任务栏保底(RIGHT_MIN)
  const maxW = window.innerWidth - 24 - 12 - RIGHT_MIN;
  return Math.max(LEFT_MIN, Math.min(v, maxW));
}
function startDrag(e: PointerEvent) {
  if (isCompact.value) return;
  e.preventDefault();
  const startX = e.clientX;
  const startW = leftW.value;
  const move = (ev: PointerEvent) => {
    const dx = ev.clientX - startX;
    leftW.value = clampLeft(startW + dx);
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    document.body.style.cursor = "";
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  document.body.style.cursor = "col-resize";
}
</script>

<template>
  <main class="shell">
    <!-- 自定义标题栏 -->
    <div class="titlebar" data-tauri-drag-region>
      <div class="tb-brand" data-tauri-drag-region>
        <img :src="logoSvg" alt="Bolt" class="tb-logo" />
        <h1>Bolt</h1>
      </div>
      <div class="tb-right">
        <button class="tb-btn" title="设置" aria-label="设置" @click="settingsOpen = true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
        </button>
        <span class="tb-sep"></span>
        <button class="tb-btn" title="最小化" aria-label="最小化" @click="minimize">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="1.8" stroke-linecap="round"><path d="M5 12h14" /></svg>
        </button>
        <button class="tb-btn" :title="maximized ? '向下还原' : '最大化'" aria-label="最大化" @click="toggleMax">
          <svg v-if="!maximized" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="1.8"><rect x="5" y="5" width="14" height="14" rx="2" /></svg>
          <svg v-else width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="1.8"><rect x="5" y="8" width="11" height="11" rx="2" /><path d="M9 5h8a2 2 0 0 1 2 2v8" /></svg>
        </button>
        <button class="tb-btn tb-close" title="关闭" aria-label="关闭" @click="closeWin">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="1.8" stroke-linecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
        </button>
      </div>
    </div>

    <div v-if="toast" class="toast">{{ toast }}</div>

    <!-- 内容区：宽屏为两栏+拖拽槽；窄屏自动单列纵向堆叠自适应 -->
    <section
      class="grid"
      :class="{ 'compact-mode': isCompact }"
      :style="isCompact ? undefined : { gridTemplateColumns: `${leftW}px 12px minmax(300px, 1fr)` }"
    >
      <div class="left-col">
        <SendPanel />
        <DevicePanel />
      </div>
      <div v-if="!isCompact" class="gutter" title="拖动调整左右分栏宽度" @pointerdown="startDrag($event)"></div>
      <TaskPanel />
    </section>

    <!-- 底部状态栏 -->
    <footer class="statusbar" :class="{ 'compact-mode': isCompact }">
      <span class="sb-item">本机：{{ localInfo.name || "—" }}</span>
      <span class="sb-sep"></span>
      <span class="sb-item" :title="localIpsText">IP：{{ localIpsText }}</span>
      <span class="sb-sep"></span>
      <span class="sb-item">在线：{{ devices.length }}台</span>
      <span class="sb-sep"></span>
      <span class="sb-item">协议：{{ protocol }}</span>
      <span class="sb-sep"></span>
      <span class="sb-item">速度：{{ fmtRate(totalRate) }}</span>
    </footer>

    <SettingsModal v-model:open="settingsOpen" />
    <Modals />
  </main>
</template>

<style>
:root {
  color-scheme: light;
  /* 设计文档色彩系统 */
  --bg: #f8fafc;
  --panel: #ffffff;
  --panel-2: #f8fafc;
  --line: #e2e8f0;
  --line-dash: #cbd5e1;
  --text: #1e293b;
  --muted: #64748b;
  --faint: #94a3b8;
  --accent: #2563eb;
  --accent-2: #3b82f6;
  --accent-soft: #eff6ff;
  --ok: #10b981;
  --warn: #f59e0b;
  --danger: #ef4444;
  /* 阴影分层（轻拟物） */
  --shadow-1: 0 8px 24px rgba(0, 0, 0, 0.06), 0 2px 6px rgba(0, 0, 0, 0.04);
  --shadow-2: 0 4px 12px rgba(0, 0, 0, 0.05);
  --shadow-hover: 0 12px 36px rgba(37, 99, 235, 0.12);
  --r-card: 18px;
  --r-item: 12px;
  --r-btn: 10px;
  --r-modal: 20px;
}
* { box-sizing: border-box; }
html, body { height: 100%; }
body {
  margin: 0;
  font-family: "Segoe UI", "Microsoft YaHei", system-ui, sans-serif;
  background: var(--bg);
  color: var(--text);
  user-select: none;
  font-size: 14px;
  overflow: hidden;
}
.shell { height: 100vh; display: flex; flex-direction: column; }

/* ---- 自定义标题栏 ---- */
.titlebar {
  height: 42px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 0 8px 0 16px;
  background: var(--panel);
  border-bottom: 1px solid var(--line);
}
.tb-brand { display: flex; align-items: center; gap: 9px; }
.tb-logo { width: 24px; height: 24px; object-fit: contain; flex-shrink: 0; }
.titlebar h1 {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0.2px;
}
.tb-ver { color: var(--faint); font-size: 12px; }
.tb-right { display: flex; align-items: center; margin-left: auto; }
.tb-sep { width: 1px; height: 16px; background: var(--line); margin: 0 6px; }
.tb-btn {
  width: 34px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  border-radius: 8px;
  color: var(--muted);
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}
.tb-btn:hover { background: var(--accent-soft); color: var(--accent); }
.tb-close:hover { background: var(--danger); color: #fff; }

/* ---- 两栏布局 ---- */
.grid {
  display: grid;
  padding: 10px 12px;
  flex: 1;
  min-height: 0;
}
.left-col {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  height: 100%;
}
/* 栏宽拖拽槽 */
.gutter { cursor: col-resize; position: relative; }
.gutter::after {
  content: "";
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 4px;
  height: 44px;
  border-radius: 2px;
  background: transparent;
  transition: background 0.15s ease;
}
.gutter:hover::after, .gutter:active::after { background: var(--line-dash); }
.panel {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-card);
  box-shadow: var(--shadow-1);
  padding: 14px;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
h2 { font-size: 15px; margin: 0; color: var(--text); font-weight: 600; }
ul { list-style: none; margin: 0; padding: 0; }

/* ---- 通用按钮（拟物立体） ---- */
.btn {
  background: linear-gradient(180deg, #ffffff, #f8fafc);
  border: 1px solid var(--line);
  color: var(--text);
  border-radius: var(--r-btn);
  padding: 7px 14px;
  cursor: pointer;
  font-size: 13px;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.04);
  transition: transform 0.12s ease, box-shadow 0.12s ease, border-color 0.12s ease, background 0.12s ease;
}
.btn:hover:not(:disabled) { transform: translateY(-2px); box-shadow: var(--shadow-2); border-color: #cbd5e1; }
.btn:active:not(:disabled) { transform: translateY(1px); box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05); }
.btn:disabled { opacity: 0.45; cursor: not-allowed; }
.btn.primary {
  background: linear-gradient(135deg, var(--accent-2), var(--accent));
  border-color: transparent;
  color: #fff;
  font-weight: 600;
  box-shadow: 0 4px 14px rgba(37, 99, 235, 0.28);
}
.btn.primary:hover:not(:disabled) { box-shadow: 0 8px 22px rgba(37, 99, 235, 0.36); }
.btn.primary:active:not(:disabled) { box-shadow: 0 2px 8px rgba(37, 99, 235, 0.3); }
.btn.sm { padding: 4px 11px; font-size: 12px; border-radius: 8px; }
.btn.danger { color: var(--danger); border-color: #fecaca; }
.btn.danger:hover:not(:disabled) { background: #fef2f2; }
.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: transparent;
  border: none;
  color: var(--muted);
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}
.icon-btn:hover { color: var(--accent); background: var(--accent-soft); }
.icon-btn:disabled { opacity: 0.5; cursor: default; }

/* ---- 状态栏 ---- */
.statusbar {
  min-height: 32px;
  height: auto;
  flex-shrink: 0;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  row-gap: 4px;
  column-gap: 12px;
  padding: 6px 16px;
  background: var(--panel-2);
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 12px;
  line-height: 1.5;
}
.statusbar span:not(.sb-sep) {
  display: inline-flex;
  align-items: center;
  word-break: break-all;
}
.statusbar .sb-sep { width: 1px; height: 12px; background: var(--line); flex-shrink: 0; }

/* ---- 其它 ---- */
.toast {
  position: fixed;
  top: 54px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--panel);
  border: 1px solid var(--line);
  padding: 8px 18px;
  border-radius: 999px;
  font-size: 13px;
  z-index: 60;
  box-shadow: var(--shadow-hover);
}
::-webkit-scrollbar { width: 8px; height: 8px; }
::-webkit-scrollbar-thumb { background: #cbd5e1; border-radius: 4px; }
::-webkit-scrollbar-thumb:hover { background: #b6c2d4; }
::-webkit-scrollbar-track { background: transparent; }

/* ---- 自适应移动端单列布局 (< 680px) ---- */
.grid.compact-mode {
  display: flex !important;
  flex-direction: column !important;
  gap: 12px !important;
  overflow-y: auto !important;
  overflow-x: hidden !important;
  grid-template-columns: none !important;
}
.grid.compact-mode .left-col {
  height: auto !important;
  min-height: auto !important;
  flex: none !important;
  display: flex !important;
  flex-direction: column !important;
  gap: 12px !important;
}
.grid.compact-mode .gutter {
  display: none !important;
}
.grid.compact-mode :deep(.panel) {
  flex: none !important;
  min-height: auto !important;
}
.grid.compact-mode :deep(.devices) {
  max-height: 280px;
}
.grid.compact-mode :deep(.tasks) {
  max-height: 280px;
}

@media (max-width: 679px) {
  .grid {
    display: flex !important;
    flex-direction: column !important;
    gap: 12px !important;
    overflow-y: auto !important;
    overflow-x: hidden !important;
    grid-template-columns: none !important;
  }
  .left-col {
    height: auto !important;
    min-height: auto !important;
    flex: none !important;
    display: flex !important;
    flex-direction: column !important;
    gap: 12px !important;
  }
  .gutter {
    display: none !important;
  }
  .grid > .panel,
  .left-col > .panel {
    flex: none !important;
    min-height: auto !important;
  }
  .devices {
    max-height: 280px !important;
  }
  .tasks {
    max-height: 280px !important;
  }
  .statusbar {
    padding: 6px 12px !important;
    row-gap: 4px !important;
    column-gap: 12px !important;
    overflow: visible !important;
  }
  .statusbar .sb-sep {
    display: none !important;
  }
}
.statusbar.compact-mode {
  padding: 6px 12px !important;
  row-gap: 4px !important;
  column-gap: 12px !important;
  overflow: visible !important;
}
.statusbar.compact-mode .sb-sep {
  display: none !important;
}
</style>
