// Bolt 桌面端类型定义（对齐 docs/ffi_api.md 事件 payload 字段）。

export interface Device {
  uuid: string;
  name: string;
  ip: string;
  quic_port: number;
  tcp_port: number;
  device_type: number;
  source: string;
}

export interface Task {
  task_id: number;
  direction: "send" | "recv";
  peer_name: string;
  file_count: number;
  total_size: number;
  done_bytes: number;
  ok_files: number;
  failed_files: number;
  state: string;
  current_file: string | null;
  /** 任务实际使用的传输协议（quic/tcp），会话建立后由后端回填 */
  transport?: string;
  /** 创建时间（unix 秒），用于完成弹窗的总耗时计算 */
  created_unix: number;
  /** 开始传输时间（毫秒） */
  start_time_ms?: number;
  /** 实际传输耗时（毫秒） */
  duration_ms?: number;
  /** 平均传输速度（字节/秒） */
  avg_rate_bps?: number;
}

/** 待发送清单项（inspect_paths 返回）。 */
export interface PendingItem {
  path: string;
  name: string;
  is_dir: boolean;
  size: number;
  files: number;
  error?: boolean;
}

export interface PairReq {
  pair_id: number;
  uuid: string;
  name: string;
  code: string;
  is_initiator?: boolean;
}

export interface TransferReq {
  req_id: number;
  uuid: string;
  name: string;
  file_count: number;
  total_size: number;
}

export interface ConnState {
  state: "connected" | "disconnected";
  transport?: string;
}

/** EVT_TASK_PROGRESS 的单文件实时进度。 */
export interface FileProgress {
  rel_path: string;
  done: number;
  total: number;
  rate_bps: number;
  eta_secs: number;
}

/** bt://event 载荷：{id: 事件常量, payload: 事件 JSON 字符串}。 */
export interface BtEvent {
  id: number;
  payload: string;
}
