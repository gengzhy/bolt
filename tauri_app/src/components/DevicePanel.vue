<script setup lang="ts">
// 设备面板（左栏下方）：【可用设备】
// 头部：标题 + 刷新 + 更多（弹窗手动输入 IP 连接）；
// 设备卡片：图标 + 名称 + 信息行 + 状态 + 连接/断开；
// 去除底端常驻输入框与多余单选按钮，悬停微立体阴影，防止边缘截断。
import { ref, watch } from "vue";
import { useBt } from "../composables/useBt";
import { targetUuid } from "../composables/selection";
import type { ConnState, Device } from "../types";

const {
  devices,
  connStates,
  localInfo,
  localIpsText,
  refreshLocalInfo,
  connect,
  disconnect,
  connectAddr,
  addManualDevice,
  probeNetwork,
  showToast,
} = useBt();

const manualModalOpen = ref(false);
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
  return t ? ` · ${t.toUpperCase()}` : "";
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
  if (targetUuid.value === d.uuid) {
    targetUuid.value = "";
  } else {
    targetUuid.value = d.uuid;
  }
}

function openManualModal() {
  void refreshLocalInfo();
  manualModalOpen.value = true;
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
  manualModalOpen.value = false;
}
async function addManual() {
  if (!validAddr()) return;
  await addManualDevice(manualIp.value.trim(), manualPort.value);
  manualModalOpen.value = false;
}
</script>

<template>
  <div class="panel">
    <div class="panel-head">
      <div class="head-title-wrap">
        <h2>可用设备</h2>
        <span class="wifi-tip">请确保目标设备连接同一 WI-FI 网络</span>
      </div>
      <div class="head-actions">
        <button class="icon-btn" :disabled="scanning" :title="scanning ? '扫描中…' : '刷新扫描'" @click="refresh()">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
            :class="{ spin: scanning }">
            <path d="M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6" />
          </svg>
        </button>
        <button class="icon-btn" title="手动输入 IP 连接" @click="openManualModal">
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
          <span class="dev-sub">{{ typeTitle(d.device_type) }} · {{ d.ip }}:{{ d.quic_port }}{{ d.source ? ` · ${d.source}` : '' }}</span>
        </span>
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
        未发现设备<br />可点击右上角刷新，或点击 “···” 手动输入 IP 连接
      </li>
    </ul>

    <!-- 手动输入 IP 连接弹窗 -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="manualModalOpen" class="modal-mask" @click="manualModalOpen = false">
          <div class="modal manual-modal" @click.stop>
            <div class="modal-header">
              <h3>手动输入 IP 连接</h3>
              <button class="icon-btn close-btn" title="关闭" @click="manualModalOpen = false">
                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                  stroke-width="2" stroke-linecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
              </button>
            </div>
            <div class="modal-body">
              <div class="field-item">
                <label>目标设备 IP 地址</label>
                <input
                  v-model="manualIp"
                  class="input-box"
                  placeholder="如 192.168.1.20"
                  @keydown.enter="directConnect"
                  autofocus
                />
              </div>
              <div class="field-item">
                <label>目标端口号</label>
                <input
                  v-model.number="manualPort"
                  type="number"
                  min="1"
                  max="65535"
                  class="input-box"
                  placeholder="8899"
                  @keydown.enter="directConnect"
                />
              </div>
              <div class="local-tip">
                本机 IP 地址：<b>{{ localIpsText }}</b>
              </div>
            </div>
            <div class="modal-actions">
              <button class="btn" @click="manualModalOpen = false">取消</button>
              <button class="btn" @click="addManual">加入列表</button>
              <button class="btn primary" @click="directConnect">直接连接</button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  gap: 8px;
}
.head-title-wrap {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
  flex: 1;
}
.head-title-wrap h2 {
  flex-shrink: 0;
}
.wifi-tip {
  font-size: 11px;
  color: var(--faint);
  white-space: nowrap;
  font-weight: normal;
  overflow: hidden;
  text-overflow: ellipsis;
}
.head-actions { display: flex; gap: 4px; flex-shrink: 0; }
.spin { animation: rot 1.1s linear infinite; }
@keyframes rot { to { transform: rotate(360deg); } }
.scanbar {
  height: 3px;
  overflow: hidden;
  margin: -4px 0 8px;
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

/* 设备列表容器：预留内边距防止悬停阴影被溢出裁剪 */
.devices {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 4px 2px;
}
.dev-card {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-item);
  padding: 10px 12px;
  cursor: pointer;
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.04);
  /* 移除 translateY(-2px)，防止向上位移被顶部边缘遮挡；采用适度柔和微立体阴影 */
  transition: border-color 0.15s ease, box-shadow 0.15s ease, background 0.15s ease;
}
.dev-card:hover {
  border-color: #cbd5e1;
  box-shadow: 0 3px 8px rgba(15, 23, 42, 0.06), 0 1px 2px rgba(15, 23, 42, 0.03);
}
.dev-card:active {
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.04);
}
.dev-card.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
  box-shadow: 0 0 0 1px var(--accent), 0 2px 6px rgba(37, 99, 235, 0.1);
}
.dev-icon {
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
.dev-card.selected .dev-icon { background: #dbeafe; }
.dev-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
.dev-name {
  font-weight: 600;
  font-size: 15px;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  word-break: break-all;
  white-space: normal;
  line-height: 1.4;
}
.dev-sub {
  color: var(--muted);
  font-size: 11.5px;
  word-break: break-all;
  white-space: normal;
  line-height: 1.45;
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
.dev-btn { flex-shrink: 0; }
.empty { color: var(--muted); font-size: 12.5px; text-align: center; padding: 26px 0; line-height: 1.7; }

.panel { flex: 1 1 0; min-height: 0; display: flex; flex-direction: column; }

/* ---- 模态弹窗 ---- */
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
}
.manual-modal {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-modal);
  padding: 20px 24px;
  width: min(380px, 92vw);
  box-shadow: 0 20px 48px rgba(15, 23, 42, 0.18), 0 4px 12px rgba(15, 23, 42, 0.08);
  display: flex;
  flex-direction: column;
  gap: 16px;
  text-align: left;
}
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.modal-header h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--text);
}
.close-btn {
  width: 28px;
  height: 28px;
}
.modal-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.field-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.field-item label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text);
}
.input-box {
  background: var(--panel-2);
  border: 1px solid var(--line);
  color: var(--text);
  border-radius: var(--r-btn);
  padding: 8px 12px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}
.input-box:focus {
  border-color: var(--accent);
  background: #ffffff;
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.local-tip {
  font-size: 11.5px;
  color: var(--muted);
  background: var(--panel-2);
  border-radius: 8px;
  padding: 6px 10px;
}
.local-tip b {
  color: var(--accent);
  font-family: Consolas, monospace;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.fade-enter-active, .fade-leave-active { transition: opacity 0.18s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
