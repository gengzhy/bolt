<script setup lang="ts">
// 发送面板（中栏，自适应）：
// 状态 A 空闲：虚线拖拽区 + 主/辅选择按钮（未选设备置灰）；
// 状态 B 拖入：浅蓝背景 + 半透玻璃浮层；
// 状态 C 已选：文件/文件夹清单（大小、数量、移除）+ 底部发送大按钮 → 发送确认弹窗。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useBt } from "../composables/useBt";
import { targetUuid } from "../composables/selection";
import { human } from "../utils/format";
import type { PendingItem } from "../types";

const { devices, sendFiles, showToast } = useBt();

const pending = ref<PendingItem[]>([]);
const dragOver = ref(false);

let unlistenDragDrop: (() => void) | undefined;

const targetName = computed(
  () => devices.value.find((d) => d.uuid === targetUuid.value)?.name ?? "",
);
const hasTarget = computed(() => targetUuid.value !== "");
const totalSize = computed(() => pending.value.reduce((s, p) => s + (p.size || 0), 0));

async function addPaths(paths: string[]) {
  const fresh = paths.filter((p) => p && !pending.value.some((x) => x.path === p));
  if (fresh.length === 0) return;
  const infos = await invoke<PendingItem[]>("inspect_paths", { paths: fresh }).catch(() => []);
  for (const p of fresh) {
    const info = infos.find((x) => x.path === p);
    pending.value.push(
      info ?? { path: p, name: p.split(/[\\/]/).pop() || p, is_dir: false, size: 0, files: 0 },
    );
  }
}
function removePath(idx: number) {
  pending.value.splice(idx, 1);
}
function clearPending() {
  pending.value = [];
}

async function pickFiles() {
  const picked = await open({ multiple: true, title: "选择要发送的文件" });
  if (!picked) return;
  await addPaths(Array.isArray(picked) ? picked : [picked]);
}
async function pickFolder() {
  const picked = await open({ directory: true, title: "选择要发送的文件夹" });
  if (!picked) return;
  await addPaths(Array.isArray(picked) ? picked : [picked]);
}

async function handleSend() {
  if (!hasTarget.value) {
    showToast("请先在下方选择目标设备");
    return;
  }
  if (pending.value.length === 0) {
    showToast("请添加要发送的文件或文件夹");
    return;
  }
  const ok = await sendFiles(targetUuid.value, pending.value.map((p) => p.path));
  if (ok) pending.value = [];
}

onMounted(async () => {
  // Tauri v2 核心 API：webview 文件拖拽事件（文件与文件夹均可拖入）
  unlistenDragDrop = await getCurrentWebview().onDragDropEvent((ev) => {
    if (ev.payload.type === "enter" || ev.payload.type === "over") {
      dragOver.value = true;
    } else if (ev.payload.type === "leave") {
      dragOver.value = false;
    } else if (ev.payload.type === "drop") {
      dragOver.value = false;
      void addPaths(ev.payload.paths ?? []);
    }
  });
});
onUnmounted(() => unlistenDragDrop?.());
</script>

<template>
  <div class="panel mid" :class="{ 'drag-over': dragOver, 'has-pending': pending.length > 0 }">
    <!-- 状态 A/B：拖拽接收区 -->
    <div v-if="pending.length === 0" class="dropzone">
      <div class="dz-inner">
        <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="currentColor"
          stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" class="dz-icon">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <path d="M17 8l-5-5-5 5M12 3v12" />
        </svg>
        <p class="dz-title">将文件或文件夹拖拽到此区域</p>
        <p class="dz-sub">支持单个文件、多文件、整个文件夹批量传输</p>
        <div class="dz-actions">
          <button class="btn primary sm dz-btn" :disabled="!hasTarget" @click="pickFiles">发送文件</button>
          <button class="btn sm dz-btn" :disabled="!hasTarget" @click="pickFolder">发送文件夹</button>
        </div>
        <p v-if="!hasTarget" class="dz-hint">请先在下方选择目标设备</p>
        <p v-else class="dz-target-hint">已选目标：<b>{{ targetName }}</b></p>
      </div>
      <!-- 状态 B：拖拽悬浮玻璃浮层 -->
      <Transition name="fade">
        <div v-if="dragOver" class="glass">
          <p>松开鼠标，添加要发送的内容</p>
        </div>
      </Transition>
    </div>

    <!-- 状态 C：已选清单 -->
    <template v-else>
      <div class="picked-head">
        <h2>待发送清单</h2>
        <button class="icon-btn" title="清空清单" @click="clearPending">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
        </button>
      </div>
      <ul class="pending">
        <li v-for="(p, i) in pending" :key="p.path" class="pend-item">
          <span class="p-icon" :class="{ dir: p.is_dir }">
            <svg v-if="p.is_dir" width="17" height="17" viewBox="0 0 24 24" fill="none"
              stroke="currentColor" stroke-width="1.8" stroke-linejoin="round">
              <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
            </svg>
            <svg v-else width="17" height="17" viewBox="0 0 24 24" fill="none"
              stroke="currentColor" stroke-width="1.8" stroke-linejoin="round">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <path d="M14 2v6h6" />
            </svg>
          </span>
          <span class="p-info">
            <span class="p-name" :title="p.path">{{ p.name }}</span>
            <span class="p-meta">
              <template v-if="p.is_dir">文件夹 · {{ p.files }} 个文件 · {{ human(p.size) }}</template>
              <template v-else>文件 · {{ human(p.size) }}</template>
            </span>
          </span>
          <button class="rm" title="移除" @click="removePath(i)">×</button>
        </li>
      </ul>
      <div class="send-foot">
        <div class="target" :class="{ none: !targetName }">
          {{ targetName ? `发送至：${targetName}` : "请先在下方选择目标设备" }}
        </div>
        <button class="btn primary send-btn" :disabled="!hasTarget" @click="handleSend">
          发送（{{ pending.length }} 项 · {{ human(totalSize) }}）
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.mid {
  display: flex;
  flex-direction: column;
  flex: 0 0 196px;
  min-height: 180px;
  gap: 10px;
  padding: 12px 14px;
  transition: flex 0.2s ease, min-height 0.2s ease;
}
.mid.has-pending {
  flex: 1 1 0;
  min-height: 200px;
}

/* ---- 状态 A/B：拖拽区 ---- */
.dropzone {
  flex: 1;
  min-height: 0;
  position: relative;
  border: 2px dashed var(--line-dash);
  border-radius: var(--r-item);
  background: var(--panel-2);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s ease, border-color 0.15s ease;
}
.drag-over .dropzone { background: var(--accent-soft); border-color: var(--accent); }
.dz-inner { text-align: center; padding: 10px 14px; position: relative; z-index: 1; }
.dz-icon { color: #94a3b8; margin-bottom: 6px; }
.drag-over .dz-icon { color: var(--accent); }
.dz-title { margin: 0 0 3px; font-size: 13.5px; font-weight: 600; color: var(--text); }
.dz-sub { margin: 0 0 8px; font-size: 11.5px; color: var(--muted); }
.dz-actions { display: flex; align-items: center; justify-content: center; gap: 8px; }
.dz-btn { padding: 5px 14px; font-size: 12px; }
.dz-hint { margin: 8px 0 0; font-size: 11.5px; color: var(--warn); }
.dz-target-hint { margin: 8px 0 0; font-size: 11.5px; color: var(--accent); }
/* 状态 B：玻璃浮层 */
.glass {
  position: absolute;
  inset: 10px;
  border-radius: 12px;
  background: rgba(239, 246, 255, 0.7);
  backdrop-filter: blur(8px);
  border: 1px solid rgba(37, 99, 235, 0.25);
  box-shadow: var(--shadow-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  pointer-events: none;
}
.glass p { margin: 0; color: var(--accent); font-size: 13px; font-weight: 600; }
.fade-enter-active, .fade-leave-active { transition: opacity 0.15s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }

/* ---- 状态 C：已选清单 ---- */
.picked-head { display: flex; align-items: center; justify-content: space-between; }
.pending {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.pend-item {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-item);
  padding: 10px 12px;
  box-shadow: var(--shadow-2);
}
.p-icon {
  width: 34px;
  height: 34px;
  flex-shrink: 0;
  border-radius: 9px;
  background: var(--accent-soft);
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
}
.p-icon.dir { background: rgba(245, 158, 11, 0.12); color: var(--warn); }
.p-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.p-name {
  font-size: 13.5px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.p-meta { font-size: 11.5px; color: var(--muted); }
.rm {
  background: none;
  border: none;
  color: var(--muted);
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
  padding: 3px 8px;
  border-radius: 6px;
  transition: color 0.12s ease, background 0.12s ease;
}
.rm:hover { color: var(--danger); background: rgba(239, 68, 68, 0.1); }

.send-foot { display: flex; flex-direction: column; gap: 8px; }
.target { text-align: center; font-size: 12px; color: var(--muted); }
.target.none { color: var(--warn); }
.send-btn { width: 100%; padding: 11px; font-size: 14px; }

/* ---- 发送确认弹窗 ---- */
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
}
.modal {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-modal);
  padding: 24px 28px;
  min-width: 340px;
  text-align: center;
  box-shadow: var(--shadow-hover), var(--shadow-1);
}
.modal h3 { margin: 0 0 12px; font-size: 16px; }
.modal p { font-size: 13px; color: var(--muted); margin: 6px 0; }
.modal p b { color: var(--text); }
.modal-actions { display: flex; justify-content: center; gap: 12px; margin-top: 18px; }
.modal-actions .btn { min-width: 96px; }
</style>
