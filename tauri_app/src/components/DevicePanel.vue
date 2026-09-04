<script setup lang="ts">
// 设备面板（左栏，280px）：【可用设备】
// 头部：标题 + 刷新 + ⋯；设备卡片：图标 + 名称 + 信息行 + 单选圆圈 + 连接/断开；
// 底部：【手动输入 IP 连接】折叠区 + 本机 IP 小字。
import { ref, watch } from "vue";
import { useLt } from "../composables/useLt";
import { targetUuid } from "../composables/selection";
import type { ConnState, Device } from "../types";

const { devices, connStates, localInfo, connect, disconnect, connectAddr, addManualDevice, probeNetwork, showToast } = useLt();

const manualOpen = ref(false);
const manualIp = ref("");
const manualPort = ref(8899);

// 刷新扫描进度条：点击即亮；扫到设备立即收起；5s 兜底熄灭
const scanning = ref(false);
let scanTimer: number | undefined;
function refresh() {
  scanning.value = true;
  window.clearTimeout(scanTimer);
  scanTimer = window.setTimeout(() => (scanning.value = false), 5000);
  probeNetwork();
}
watch(devices, (list) => {
  if (scanning.value && list.length > 0) {
    scanning.value = false;
    window.clearTimeout(scanTimer);
  }
});

function conn(uuid: string): ConnState | undefined {
  return (connStates as Record<string, ConnState>)[uuid];
}
function transportOf(uuid: string): string {
  const t = conn(uuid)?.transport;
  return t ? ` · ${t}` : "";
}

// 设备类型图标（PC / Android / iPhone）
function typeIcon(t: number): string {
  if (t === 1) return "M2 4h20v13H2zM8 21h8M12 17v4";
  if (t === 2 || t === 3) return "M7 2h10a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1zM11 18h2";
  return "M2 4h20v13H2zM8 21h8M12 17v4";
}
function typeTitle(t: number): string {
  switch (t) {
    case 1: return "电脑（Windows）";
    case 2: return "手机（Android）";
    case 3: return "手机（iOS）";
    default: return "设备";
  }
}

function select(d: Device) {
  targetUuid.value = d.uuid;
}

function validAddr(): boolean {
  if (!manualIp.value.trim()) {
    showToast("请填写 IP 地址");
    return false;
  }
  return true;
}
async function directConnect() {
  if (!validAddr()) return;
  await connectAddr(manualIp.value.trim(), manualPort.value);
  manualOpen.value = false;
}
async function addManual() {
  if (!validAddr()) return;
  await addManualDevice(manualIp.value.trim(), manualPort.value);
  manualOpen.value = false;
}
</script>

<template>
  <div class="panel">
    <div class="panel-head">
      <h2>可用设备</h2>
      <div class="head-actions">
        <button class="icon-btn" :disabled="scanning" :title="scanning ? '扫描中…' : '刷新扫描'" @click="refresh()">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
            :class="{ spin: scanning }">
            <path d="M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6" />
          </svg>
        </button>
        <button class="icon-btn" :title="manualOpen ? '收起手动连接' : '更多'" @click="manualOpen = !manualOpen">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor">
            <circle cx="5" cy="12" r="1.6" /><circle cx="12" cy="12" r="1.6" /><circle cx="19" cy="12" r="1.6" />
          </svg>
        </button>
      </div>
    </div>
    <div v-if="scanning" class="scanbar"><div class="scanbar-ind"></div></div>

    <ul class="devices">
      <li
        v-for="d in devices"
        :key="d.uuid"
        class="dev-card"
        :class="{ selected: targetUuid === d.uuid }"
        @click="select(d)"
      >
        <span class="dev-icon">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <path :d="typeIcon(d.device_type)" />
          </svg>
        </span>
        <span class="dev-info">
          <span class="dev-name">
            {{ d.name }}
            <span v-if="conn(d.uuid)" class="conn-badge">已连接{{ transportOf(d.uuid) }}</span>
          </span>
          <span class="dev-sub">{{ typeTitle(d.device_type) }} · {{ d.ip }} · QUIC:{{ d.quic_port }} · {{ d.source }}</span>
        </span>
        <span class="radio" :class="{ on: targetUuid === d.uuid }"></span>
        <button
          v-if="conn(d.uuid)"
          class="btn sm danger dev-btn"
          @click.stop="disconnect(d.uuid)"
        >
          断开
        </button>
        <button v-else class="btn sm dev-btn" @click.stop="connect(d.uuid)">连接</button>
      </li>
      <li v-if="devices.length === 0" class="empty">
        未发现设备<br />点刷新扫描，或在下方手动输入 IP 连接
      </li>
    </ul>

    <div class="panel-foot">
      <button class="manual-toggle" @click="manualOpen = !manualOpen">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round">
          <path v-if="manualOpen" d="M18 15l-6-6-6 6" />
          <path v-else d="M6 9l6 6 6-6" />
        </svg>
        手动输入 IP 连接
      </button>
      <div v-if="manualOpen" class="addr-form">
        <input v-model="manualIp" placeholder="IP 地址，如 192.168.1.20" />
        <input v-model.number="manualPort" type="number" min="1" max="65535" class="port" placeholder="端口" />
        <div class="addr-actions">
          <button class="btn sm primary" @click="directConnect">直连</button>
          <button class="btn sm" @click="addManual">加入列表</button>
        </div>
      </div>
      <div class="local-ip">本机IP: {{ localInfo.ips[0] || "—" }}</div>
    </div>
  </div>
</template>

<style scoped>
.panel-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
.head-actions { display: flex; gap: 2px; }
.spin { animation: rot 1.1s linear infinite; }
@keyframes rot { to { transform: rotate(360deg); } }
.scanbar {
  height: 3px;
  overflow: hidden;
  margin: -6px 0 10px;
  background: var(--panel-2);
  border-radius: 2px;
}
.scanbar-ind {
  width: 40%;
  height: 100%;
  background: var(--accent);
  border-radius: 2px;
  animation: scan-move 1.1s linear infinite;
}
@keyframes scan-move {
  from { margin-left: -40%; }
  to { margin-left: 100%; }
}
.devices { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; }
.dev-card {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-item);
  padding: 10px 12px;
  cursor: pointer;
  box-shadow: var(--shadow-2);
  transition: transform 0.12s ease, border-color 0.12s ease, box-shadow 0.12s ease, background 0.12s ease;
}
.dev-card:hover { transform: translateY(-2px); box-shadow: var(--shadow-hover); }
.dev-card:active { transform: translateY(1px); }
.dev-card.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.dev-icon {
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  border-radius: 10px;
  background: var(--accent-soft);
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
}
.dev-card.selected .dev-icon { background: #dbeafe; }
.dev-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
.dev-name {
  font-weight: 600;
  font-size: 13.5px;
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
}
.dev-name > * { min-width: 0; }
.dev-sub {
  color: var(--muted);
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.conn-badge {
  font-size: 10.5px;
  font-weight: 400;
  padding: 1px 7px;
  border-radius: 999px;
  background: rgba(16, 185, 129, 0.12);
  color: var(--ok);
  flex-shrink: 0;
}
/* 单选圆圈 */
.radio {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  border-radius: 50%;
  border: 2px solid #cbd5e1;
  position: relative;
  transition: border-color 0.12s ease;
}
.radio.on { border-color: var(--accent); }
.radio.on::after {
  content: "";
  position: absolute;
  inset: 3px;
  border-radius: 50%;
  background: var(--accent);
}
.dev-btn { flex-shrink: 0; }
.empty { color: var(--muted); font-size: 12.5px; text-align: center; padding: 26px 0; line-height: 1.7; }

.panel-foot { margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--line); }
.manual-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  background: none;
  border: none;
  color: var(--accent);
  font-size: 13px;
  font-weight: 600;
  padding: 6px 2px;
  cursor: pointer;
  border-radius: 8px;
}
.manual-toggle:hover { background: var(--accent-soft); }
.addr-form { display: flex; flex-direction: column; gap: 8px; padding: 8px 2px 4px; }
.addr-form input {
  background: var(--panel-2);
  border: 1px solid var(--line);
  color: var(--text);
  border-radius: var(--r-btn);
  padding: 7px 10px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.12s ease;
}
.addr-form input:focus { border-color: var(--accent); }
.addr-actions { display: flex; gap: 8px; }
.addr-actions .btn { flex: 1; }
.local-ip { color: var(--faint); font-size: 12px; margin-top: 8px; padding-left: 2px; }
</style>
