// 展示格式化工具。

export function human(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(1)}${units[i]}`;
}

export function humanRate(bps: number): string {
  return `${human(bps)}/s`;
}

export function humanEta(secs: number): string {
  if (!Number.isFinite(secs) || secs <= 0) return "--";
  const m = Math.floor(secs / 60);
  const s = Math.round(secs % 60);
  return m > 0 ? `${m}分${s}秒` : `${s}秒`;
}

const STATE_LABEL: Record<string, string> = {
  waiting_accept: "等待接受",
  transferring: "传输中",
  paused: "已暂停",
  done: "完成",
  cancelled: "已取消",
  error: "错误",
  rejected: "被拒绝",
};

export function stateLabel(s: string): string {
  return STATE_LABEL[s] ?? s;
}
