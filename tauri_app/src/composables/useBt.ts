// useBt：bt-ffi 命令封装 + bt://event 事件分发（模块级单例，各组件共享同一份状态）。
//
// - 事件常量对齐 docs/ffi_api.md：1 设备列表 / 2 连接状态 / 3 配对 /
//   4 传输请求 / 5 任务状态 / 6 任务进度 / 7 任务汇总 / 8 错误。
// - 后端命令返回负错误码时转为中文提示（对齐 protocol_spec.md §7）。

import { ref, reactive, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConnState,
  Device,
  FileProgress,
  BtEvent,
  PairReq,
  Task,
  TransferReq,
} from "../types";
import { useTransferLogs } from "./useTransferLogs";

// ---------- 错误码文案（protocol_spec.md §7） ----------
const ERR_TEXT: Record<number, string> = {
  [-1]: "参数无效",
  [-2]: "端口不可用",
  [-3]: "超时/对端离线",
  [-4]: "文件不可访问",
  [-5]: "磁盘空间不足",
  [-6]: "校验和不匹配",
  [-7]: "发现服务不可用",
  [-8]: "内存映射失败",
  [-9]: "已取消",
  [-10]: "权限不足",
  [-11]: "配对失败",
  [-12]: "传输被拒绝",
  [-13]: "协议不兼容",
  [-14]: "对方指纹已变更（可能非原设备）",
  [-15]: "内部错误",
};

export function errText(e: unknown): string {
  if (typeof e === "number") return ERR_TEXT[e] ?? `错误(${e})`;
  return String(e);
}

// ---------- 模块级单例状态 ----------
const devices = ref<Device[]>([]);
const tasks = ref<Task[]>([]);
const connStates = reactive<Record<string, ConnState>>({});
/** task_id → 当前文件实时进度（EVT_TASK_PROGRESS 增量更新）。 */
const progress = reactive<Record<number, FileProgress>>({});
const pairReq = ref<PairReq | null>(null);
const transReq = ref<TransferReq | null>(null);
const toast = ref("");
const version = ref("");
const fingerprint = ref("");
/** 本机信息（bt_get_local_info）：设备名、全部非环回 IPv4 与实际监听端口。 */
const localInfo = ref<{ name: string; ips: string[]; qport: number }>({
  name: "",
  ips: [],
  qport: 0,
});

const localIpsText = computed(() => {
  const ips = localInfo.value.ips;
  return ips.length > 0 ? ips.join("、") : "—";
});

let toastTimer: number | undefined;
let unlisten: UnlistenFn | undefined;

export function useBt() {
  // ---------- 基础 ----------
  function showToast(msg: string) {
    toast.value = msg;
    if (toastTimer) window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => (toast.value = ""), 3000);
  }

  async function refreshDevices() {
    devices.value = await invoke<Device[]>("get_devices");
  }
  const { recordTask } = useTransferLogs();

  async function refreshTasks() {
    const list = await invoke<Task[]>("get_tasks");
    tasks.value = list;
    for (const t of list) {
      recordTask(t);
    }
  }
  async function refreshLocalInfo() {
    const info = await invoke<Record<string, unknown>>("get_local_info").catch(() => null);
    if (info) {
      localInfo.value = {
        name: typeof info.name === "string" ? info.name : "",
        ips: Array.isArray(info.ips) ? (info.ips as string[]) : [],
        qport: typeof info.qport === "number" ? info.qport : 0,
      };
    }
  }
  async function refresh() {
    await Promise.all([refreshDevices(), refreshTasks(), refreshLocalInfo()]);
  }

  // ---------- 事件分发 ----------
  function onEvent(ev: BtEvent) {
    let p: Record<string, unknown> = {};
    try {
      p = JSON.parse(ev.payload);
    } catch {
      /* ignore */
    }
    switch (ev.id) {
      case 1: // EVT_DEVICE_LIST
        void refreshDevices();
        break;
      case 2: {
        // EVT_CONN_STATE：uuid → 连接状态/传输通道
        const uuid = p.uuid as string;
        if (uuid) {
          const state = (p.state as ConnState["state"]) ?? "disconnected";
          if (state === "connected") {
            connStates[uuid] = {
              state,
              transport: (p.transport as string | undefined) ?? undefined,
            };
          } else {
            // 断开后移除条目，UI 回落到「连接」按钮（残留 disconnected 条目
            // 会让「已连接」徽标与「断开」按钮卡死）
            delete connStates[uuid];
          }
          // 连接建立或断开时，若存在当前设备的配对弹窗，自动解除并关闭弹窗
          if (pairReq.value && pairReq.value.uuid === uuid) {
            pairReq.value = null;
          }
        }
        if (p.state === "connected") {
          showToast(`已连接 ${p.name ?? uuid}（${p.transport ?? "?"}）`);
        } else if (p.err) {
          showToast(`连接断开：${p.err}`);
        } else {
          showToast("已断开");
        }
        break;
      }
      case 3: // EVT_PAIR_REQUEST
        pairReq.value = {
          pair_id: p.pair_id as number,
          uuid: p.uuid as string,
          name: p.name as string,
          code: p.code as string,
          is_initiator: Boolean(p.is_initiator),
        };
        break;
      case 4: // EVT_TRANSFER_REQUEST
        transReq.value = {
          req_id: p.req_id as number,
          uuid: p.uuid as string,
          name: p.name as string,
          file_count: p.file_count as number,
          total_size: p.total_size as number,
        };
        void refreshTasks();
        break;
      case 5: // EVT_TASK_STATE
      case 7: // EVT_TASK_SUMMARY
        if (ev.id === 7 || (p.state as string) === "done") {
          delete progress[p.task_id as number];
        } else if ((p.state as string) === "paused") {
          // 【核心修复：暂停时立即归零速率与预估时间】：
          // 彻底消除卡片停留在上一个采样周期的瞬时速率与残留 ETA
          const id = p.task_id as number;
          if (progress[id]) {
            progress[id].rate_bps = 0;
            progress[id].eta_secs = 0;
          }
          const t = tasks.value.find((x) => x.task_id === id);
          if (t) {
            t.state = "paused";
            t.rate_bps = 0;
            t.eta_secs = 0;
          }
        }
        void refreshTasks();
        break;
      case 6: {
        // EVT_TASK_PROGRESS：增量更新单文件进度，避免整表轮询
        const id = p.task_id as number;
        progress[id] = {
          rel_path: (p.rel_path as string) ?? "",
          done: (p.done as number) ?? 0,
          total: (p.total as number) ?? 0,
          rate_bps: (p.rate_bps as number) ?? 0,
          eta_secs: (p.eta_secs as number) ?? 0,
        };
        const t = tasks.value.find((x) => x.task_id === id);
        if (t) {
          t.current_file = progress[id].rel_path;
          // 进度条/百分比直接由进度事件驱动（与后端记录同语义），实时可见
          t.done_bytes = progress[id].done;
          // 【核心修复：严格守护非传输态】：
          // 暂停（paused）、已完成（done）、出错（error）、取消（cancelled）等终态/挂起态
          // 严禁被滞后的进度事件覆写为 transferring！仅当处于 waiting_accept 时才允许扭转
          if (t.state === "paused") {
            progress[id].rate_bps = 0;
            progress[id].eta_secs = 0;
            t.rate_bps = 0;
            t.eta_secs = 0;
          } else if (t.state === "waiting_accept") {
            t.state = "transferring";
          }
        } else {
          // 任务行尚未入列（错过建档事件）→ 拉一次全量补齐
          void refreshTasks();
        }
        break;
      }
      case 8: // EVT_ERROR
        showToast(`错误(${p.code}): ${p.message ?? ""}`);
        if (pairReq.value) {
          pairReq.value = null;
        }
        void refreshTasks();
        break;
      default:
        break;
    }
  }

  async function start() {
    if (!unlisten) {
      unlisten = await listen("bt://event", (ev) => onEvent(ev.payload as BtEvent));
    }
    fingerprint.value = await invoke<string>("get_local_fingerprint");
    version.value = await invoke<string>("version");
    const info = await invoke<Record<string, unknown>>("get_local_info").catch(
      () => null,
    );
    if (info) {
      localInfo.value = {
        name: typeof info.name === "string" ? info.name : "",
        ips: Array.isArray(info.ips) ? (info.ips as string[]) : [],
        qport: typeof info.qport === "number" ? info.qport : 0,
      };
    }
    await refresh();
  }
  function stop() {
    unlisten?.();
    unlisten = undefined;
  }

  // ---------- 操作命令（负错误码 → 中文提示） ----------
  async function connect(uuid: string): Promise<boolean> {
    try {
      await invoke("connect", { uuid });
      return true;
    } catch (e) {
      showToast(`连接失败：${errText(e)}`);
      return false;
    }
  }
  async function connectAddr(ip: string, port: number): Promise<boolean> {
    try {
      await invoke("connect_addr", { ip, port });
      return true;
    } catch (e) {
      showToast(`直连失败：${errText(e)}`);
      return false;
    }
  }
  async function disconnect(uuid: string) {
    try {
      await invoke("disconnect", { uuid });
    } catch (e) {
      showToast(`断开失败：${errText(e)}`);
    }
  }
  async function sendFiles(uuid: string, paths: string[]): Promise<boolean> {
    try {
      const taskId = await invoke<number>("send_files", { uuid, paths });
      showToast(`已创建发送任务 ${taskId}`);
      return true;
    } catch (e) {
      showToast(`发送失败：${errText(e)}`);
      return false;
    }
  }
  async function cancelTask(id: number) {
    try {
      await invoke("cancel_task", { taskId: id });
    } catch (e) {
      showToast(`取消失败：${errText(e)}`);
    }
  }
  async function respondPair(pairId: number, accept: boolean) {
    await invoke("respond_pair", { pairId, accept });
    pairReq.value = null;
  }
  async function respondTransfer(reqId: number, accept: boolean) {
    await invoke("respond_transfer", { reqId, accept });
    transReq.value = null;
  }
  async function loadConfig(): Promise<Record<string, unknown>> {
    return await invoke<Record<string, unknown>>("get_config");
  }
  async function saveConfig(patch: Record<string, unknown>, silent = false): Promise<boolean> {
    try {
      await invoke("set_config", { json: JSON.stringify(patch) });
      if (!silent) showToast("设置已保存");
      return true;
    } catch (e) {
      showToast(`保存设置失败：${errText(e)}`);
      return false;
    }
  }
  function probeNetwork() {
    void invoke("probe_network");
    showToast("正在扫描局域网…");
  }
  async function addManualDevice(ip: string, port: number): Promise<boolean> {
    try {
      await invoke("add_manual_device", { ip, port });
      showToast("已加入设备列表");
      await refreshDevices();
      return true;
    } catch (e) {
      showToast(`添加失败：${errText(e)}`);
      return false;
    }
  }
  async function clearRecords() {
    await invoke("clear_records");
    await refreshTasks();
    showToast("传输记录已清除");
  }
  async function clearTempCache() {
    try {
      await invoke("clear_temp_cache");
      showToast("临时缓存已清理");
    } catch (e) {
      showToast(`清理失败：${errText(e)}`);
    }
  }
  /** 资源管理器中定位接收文件（接收侧「打开文件夹」）。 */
  async function revealPath(path: string) {
    try {
      await invoke("reveal_path", { path });
    } catch (e) {
      showToast(`打开失败：${errText(e)}`);
    }
  }

  return {
    // 状态
    devices,
    tasks,
    connStates,
    progress,
    pairReq,
    transReq,
    toast,
    version,
    fingerprint,
    localInfo,
    localIpsText,
    // 生命周期
    start,
    stop,
    refresh,
    refreshLocalInfo,
    showToast,
    // 操作
    connect,
    connectAddr,
    disconnect,
    sendFiles,
    cancelTask,
    respondPair,
    respondTransfer,
    loadConfig,
    saveConfig,
    probeNetwork,
    addManualDevice,
    clearRecords,
    clearTempCache,
    revealPath,
  };
}
