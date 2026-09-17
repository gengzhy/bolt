// useTransferLogs：传输历史记录日志持久化管理（模块级单例，基于 localStorage）。
// 即使用户在「传输任务」中点击全部清除，此处的历史审计日志仍然完好保留。

import { ref } from "vue";
import type { Task } from "../types";

export interface TransferLogItem {
  id: string;
  task_id: number;
  direction: "send" | "recv";
  transport: string;
  peer_name: string;
  file_name: string;
  file_count: number;
  total_size: number;
  avg_rate_bps: number;
  start_time_ms: number;
  end_time_ms: number;
  duration_ms: number;
  state: string;
  ok_files: number;
  failed_files: number;
}

const STORAGE_KEY = "bolt_transfer_logs";
const MAX_LOGS_COUNT = 500;

function loadFromStorage(): TransferLogItem[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveToStorage(list: TransferLogItem[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch (e) {
    console.error("Failed to save transfer logs to localStorage", e);
  }
}

// 模块级单例状态
const logs = ref<TransferLogItem[]>(loadFromStorage());
const logsOpen = ref(false);

export function useTransferLogs() {
  function recordTask(t: Task) {
    const list = [...logs.value];
    const logId = `task_${t.task_id}_${t.created_unix || 0}`;

    const startMs = t.start_time_ms && t.start_time_ms > 0
      ? t.start_time_ms
      : (t.created_unix ? t.created_unix * 1000 : Date.now());

    let endMs = 0;
    if (t.duration_ms && t.duration_ms > 0) {
      endMs = startMs + t.duration_ms;
    } else if (["done", "error", "cancelled"].includes(t.state)) {
      endMs = Date.now();
    }

    const fileName = t.current_file && t.current_file.trim().length > 0
      ? t.current_file
      : (t.file_count > 1 ? `${t.file_count} 个文件` : `任务 #${t.task_id}`);

    let rate = t.avg_rate_bps || 0;
    if (rate <= 0 && t.duration_ms && t.duration_ms > 0 && t.total_size > 0) {
      rate = Math.round((t.total_size * 1000) / t.duration_ms);
    }
    if (rate <= 0 && t.rate_bps) {
      rate = t.rate_bps;
    }

    const item: TransferLogItem = {
      id: logId,
      task_id: t.task_id,
      direction: t.direction,
      transport: t.transport || "",
      peer_name: t.peer_name || "",
      file_name: fileName,
      file_count: t.file_count || 1,
      total_size: t.total_size || t.done_bytes || 0,
      avg_rate_bps: rate,
      start_time_ms: startMs,
      end_time_ms: endMs,
      duration_ms: t.duration_ms || 0,
      state: t.state,
      ok_files: t.ok_files || 0,
      failed_files: t.failed_files || 0,
    };

    const idx = list.findIndex((x) => x.task_id === t.task_id);
    if (idx >= 0) {
      const old = list[idx];
      const preservedStart = old.start_time_ms > 0 && old.start_time_ms < startMs ? old.start_time_ms : startMs;
      list[idx] = { ...item, start_time_ms: preservedStart };
    } else {
      list.unshift(item);
    }

    list.sort((a, b) => b.start_time_ms - a.start_time_ms);
    const capped = list.slice(0, MAX_LOGS_COUNT);
    logs.value = capped;
    saveToStorage(capped);
  }

  function clearLogs() {
    logs.value = [];
    saveToStorage([]);
  }

  function openLogs() {
    logsOpen.value = true;
  }

  function closeLogs() {
    logsOpen.value = false;
  }

  return {
    logs,
    logsOpen,
    recordTask,
    clearLogs,
    openLogs,
    closeLogs,
  };
}
