<script setup lang="ts">
// 任务面板（右栏，300px）：【传输任务】
// 任务项：方向图标 + 对端/文件名 + 速度/进度 + 8px 圆角进度条（蓝/绿/红/橙）+ 暂停|取消|重传；
// 头部 ⋯ 菜单（全部清除/清理临时缓存）；底部汇总（总速度、剩余时间）+ 全部清除。
import { computed, ref } from "vue";
import { useLt } from "../composables/useLt";
import { human, humanEta, humanRate } from "../utils/format";
import type { Task } from "../types";

const { tasks, progress, cancelTask, clearRecords, clearTempCache, loadConfig, revealPath } = useLt();

const menuOpen = ref(false);

/** 接收侧「打开文件夹」：资源管理器定位接收文件（保存目录 + 相对路径）。 */
async function openFolder(t: Task) {
  const cfg = await loadConfig();
  const dir = String(cfg.save_dir ?? "");
  const rel = (t.current_file ?? "").replace(/\//g, "\\");
  const full = dir && rel ? `${dir.replace(/[\\/]+$/, "")}\\${rel}` : dir;
  await revealPath(full);
}

function pct(done: number, total: number): string {
  return total > 0 ? `${Math.min(100, Math.round((done / total) * 100))}%` : "";
}
function barClass(state: string): string {
  if (state === "done") return "ok";
  if (state === "error") return "err";
  if (state === "cancelled") return "warn";
  return "run";
}
function stateText(t: { state: string; done_bytes: number; total_size: number }): string {
  switch (t.state) {
    case "done": return "完成";
    case "error": return "失败";
    case "cancelled": return "已取消";
    case "waiting_accept": return "等待接受";
    default: {
      const p = pct(t.done_bytes, t.total_size);
      return p ? `传输中 ${p}` : "传输中";
    }
  }
}

const activeTasks = computed(() => tasks.value.filter((t) => progress[t.task_id]));
const totalRate = computed(() => activeTasks.value.reduce((s, t) => s + progress[t.task_id].rate_bps, 0));
const totalEta = computed(() => {
  let m = 0;
  for (const t of activeTasks.value) m = Math.max(m, progress[t.task_id].eta_secs);
  return m;
});

function taskAvgRate(t: Task): number {
  if (t.avg_rate_bps && t.avg_rate_bps > 0) return t.avg_rate_bps;
  if (t.duration_ms && t.duration_ms > 0 && t.total_size > 0) {
    return Math.round((t.total_size * 1000) / t.duration_ms);
  }
  return 0;
}

function menuAction(fn: () => void) {
  menuOpen.value = false;
  fn();
}
</script>

<template>
  <div class="panel">
    <div class="panel-head">
      <h2>传输任务</h2>
      <div class="menu-wrap">
        <button class="icon-btn" title="更多" @click="menuOpen = !menuOpen">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor">
            <circle cx="5" cy="12" r="1.6" /><circle cx="12" cy="12" r="1.6" /><circle cx="19" cy="12" r="1.6" />
          </svg>
        </button>
        <div v-if="menuOpen" class="menu">
          <button @click="menuAction(clearRecords)">全部清除</button>
          <button @click="menuAction(clearTempCache)">清理临时缓存</button>
        </div>
      </div>
    </div>

    <ul class="tasks" @click="menuOpen = false">
      <li v-for="t in tasks" :key="t.task_id" class="task-item">
        <div class="task-head">
          <span class="t-icon" :class="t.direction">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
              stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path v-if="t.direction === 'send'" d="M12 19V5M5 12l7-7 7 7" />
              <path v-else d="M12 5v14M19 12l-7 7-7-7" />
            </svg>
          </span>
          <span class="t-name" :title="t.peer_name">
            {{ t.peer_name || "-" }}
            <span class="t-dir">{{ t.direction === "send" ? "发出" : "接收" }}</span>
          </span>
          <span v-if="t.transport" class="t-proto" :title="`传输协议：${t.transport.toUpperCase()}`">
            {{ t.transport.toUpperCase() }}
          </span>
          <span class="t-state" :class="barClass(t.state)">{{ stateText(t) }}</span>
        </div>
        <div class="task-file">
          {{ t.current_file || `${t.file_count} 个文件 · 共 ${human(t.total_size)}` }}
        </div>
        <div v-if="progress[t.task_id]" class="task-rate">
          {{ humanRate(progress[t.task_id].rate_bps) }} · 剩余 {{ humanEta(progress[t.task_id].eta_secs) }}
        </div>
        <div v-if="t.total_size > 0" class="bar">
          <div
            class="bar-fill"
            :class="barClass(t.state)"
            :style="{ width: Math.min(100, (t.done_bytes / t.total_size) * 100) + '%' }"
          ></div>
        </div>
        <div class="task-actions">
          <span v-if="t.state === 'transferring'">
            {{ human(t.done_bytes) }} / {{ human(t.total_size) }} · 成功 {{ t.ok_files }} 失败 {{ t.failed_files }}
          </span>
          <span v-else>
            {{ human(t.total_size) }} · {{ humanRate(taskAvgRate(t)) }} · 成功 {{ t.ok_files }} 失败 {{ t.failed_files }}
          </span>
          <span class="spacer"></span>
          <button
            v-if="['transferring', 'waiting_accept'].includes(t.state)"
            class="btn sm danger"
            @click="cancelTask(t.task_id)"
          >
            取消
          </button>
          <button
            v-if="t.direction === 'recv' && t.state === 'done'"
            class="btn sm"
            @click="openFolder(t)"
          >
            打开文件夹
          </button>
        </div>
      </li>
      <li v-if="tasks.length === 0" class="empty">暂无传输任务</li>
    </ul>

    <div class="panel-foot">
      <div class="summary">
        <span>总速度 {{ humanRate(totalRate) }}</span>
        <span>剩余 {{ humanEta(totalEta) }}</span>
      </div>
      <button class="btn clear-btn" @click="clearRecords">全部清除</button>
    </div>
  </div>
</template>

<style scoped>
.panel-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
.menu-wrap { position: relative; }
.menu {
  position: absolute;
  right: 0;
  top: 34px;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-item);
  box-shadow: var(--shadow-hover);
  padding: 5px;
  display: flex;
  flex-direction: column;
  min-width: 130px;
  z-index: 20;
}
.menu button {
  background: none;
  border: none;
  text-align: left;
  padding: 7px 10px;
  font-size: 13px;
  color: var(--text);
  border-radius: 8px;
  cursor: pointer;
}
.menu button:hover { background: var(--accent-soft); color: var(--accent); }

.tasks { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; }
.task-item {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-item);
  padding: 10px 12px;
  box-shadow: var(--shadow-2);
  display: flex;
  flex-direction: column;
}
.task-head { display: flex; align-items: center; gap: 8px; }
.t-icon {
  width: 28px;
  height: 28px;
  flex-shrink: 0;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.t-icon.send { background: var(--accent-soft); color: var(--accent); }
.t-icon.recv { background: rgba(16, 185, 129, 0.12); color: var(--ok); }
.t-name {
  flex: 1;
  min-width: 0;
  font-size: 13.5px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.t-dir {
  font-size: 10.5px;
  font-weight: 400;
  color: var(--muted);
  margin-left: 4px;
}
.t-state { font-size: 12px; font-weight: 600; flex-shrink: 0; }
.t-proto {
  font-size: 10px;
  font-weight: 700;
  color: var(--accent);
  background: var(--accent-soft);
  border-radius: 4px;
  padding: 1px 5px;
  flex-shrink: 0;
  letter-spacing: 0.5px;
}
.t-state.run { color: var(--accent); }
.t-state.ok { color: var(--ok); }
.t-state.err { color: var(--danger); }
.t-state.warn { color: var(--warn); }
.task-file {
  color: var(--muted);
  font-size: 12px;
  margin-top: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.task-rate { color: var(--muted); font-size: 12px; margin-top: 2px; }
.bar { height: 8px; background: var(--line); border-radius: 8px; overflow: hidden; margin-top: 8px; }
.bar-fill { height: 100%; border-radius: 8px; transition: width 0.3s; }
.bar-fill.run { background: linear-gradient(90deg, var(--accent-2), var(--accent)); }
.bar-fill.ok { background: var(--ok); }
.bar-fill.err { background: var(--danger); }
.bar-fill.warn { background: var(--warn); }
.task-actions { display: flex; align-items: center; gap: 6px; margin-top: 8px; color: var(--muted); font-size: 11.5px; }
.spacer { flex: 1; }
.empty { color: var(--muted); font-size: 12.5px; text-align: center; padding: 26px 0; }

.panel-foot { margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--line); display: flex; flex-direction: column; gap: 8px; }
.summary { display: flex; justify-content: space-between; color: var(--muted); font-size: 12px; padding: 0 2px; }
.clear-btn { width: 100%; }
</style>
