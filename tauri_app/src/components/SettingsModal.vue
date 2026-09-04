<script setup lang="ts">
// 设置弹窗：标题栏齿轮唤起，自右侧滑入的独立浮层卡片（20px 圆角玻璃拟物）。
// 分组：网络传输 / 文件接收 / 设备发现 / 本机设备 / 本机信息 / 维护 / 关于。
import { onMounted, onUnmounted, ref, watch } from "vue";
import { open as pickDialog } from "@tauri-apps/plugin-dialog";
import { useLt } from "../composables/useLt";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "update:open", v: boolean): void }>();

const { fingerprint, localInfo, version, loadConfig, saveConfig, clearRecords, clearTempCache } = useLt();

const loaded = ref(false);
// 网络传输
const preferQuic = ref(true);
const concurrency = ref(4);
const chunkSize = ref(1024 * 1024);
// 文件接收
const saveDir = ref("");
const collision = ref("rename");
// 设备发现
const useMdns = ref(true);
const listenPort = ref(8899);
// 本机设备
const deviceName = ref("");
const stealthMode = ref(false);
const autoAcceptTrusted = ref(false);

function close() {
  emit("update:open", false);
}

watch(
  () => props.open,
  async (v) => {
    if (v && !loaded.value) {
      const cfg = await loadConfig();
      preferQuic.value = cfg.prefer_quic !== false;
      concurrency.value = Number(cfg.concurrency) || 4;
      chunkSize.value = Number(cfg.chunk_size) || 1024 * 1024;
      saveDir.value = (cfg.save_dir as string) ?? "";
      collision.value = typeof cfg.collision === "string" ? cfg.collision : "rename";
      useMdns.value = cfg.use_mdns !== false;
      listenPort.value = Number(cfg.listen_port) || 8899;
      deviceName.value = (cfg.device_name as string) ?? "";
      stealthMode.value = Boolean(cfg.stealth_mode);
      autoAcceptTrusted.value = Boolean(cfg.auto_accept_trusted);
      loaded.value = true;
    }
  },
);

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && props.open) close();
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));

async function pickSaveDir() {
  const picked = await pickDialog({ directory: true, title: "选择接收文件保存目录" });
  if (picked && !Array.isArray(picked)) saveDir.value = picked;
}

async function save() {
  const c = Math.min(16, Math.max(1, Math.round(concurrency.value) || 4));
  concurrency.value = c;
  const ok = await saveConfig({
    prefer_quic: preferQuic.value,
    concurrency: c,
    chunk_size: chunkSize.value,
    save_dir: saveDir.value,
    collision: collision.value,
    use_mdns: useMdns.value,
    listen_port: listenPort.value,
    device_name: deviceName.value,
    stealth_mode: stealthMode.value,
    auto_accept_trusted: autoAcceptTrusted.value,
  });
  if (ok) close();
}
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="props.open" class="backdrop" @click="close"></div>
    </Transition>
    <Transition name="slide">
      <aside v-if="props.open" class="settings-modal">
        <header>
          <h2 class="title">设置</h2>
          <button class="icon-btn" title="关闭" aria-label="关闭" @click="close">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
              stroke-width="1.8" stroke-linecap="round">
              <path d="M18 6 6 18M6 6l12 12" />
            </svg>
          </button>
        </header>

        <div class="body">
          <section class="group">
            <h3 class="group-title">网络传输设置</h3>
            <label class="row">
              <span>传输协议</span>
              <select v-model="preferQuic">
                <option :value="true">QUIC（默认）</option>
                <option :value="false">TCP</option>
              </select>
            </label>
            <p class="group-note">两端任一设备选择 TCP，连接即使用 TCP；两端都选 QUIC 才用 QUIC。已建立的连接仍使用原协议；所选协议不可用时直接提示连接失败，不自动切换。</p>
            <label class="row">
              <span>并发流数量（1-16）</span>
              <input v-model.number="concurrency" type="number" min="1" max="16" class="num" />
            </label>
            <label class="row">
              <span>分片大小</span>
              <select v-model.number="chunkSize">
                <option :value="256 * 1024">256KB</option>
                <option :value="512 * 1024">512KB</option>
                <option :value="1024 * 1024">1MB</option>
                <option :value="4 * 1024 * 1024">4MB</option>
              </select>
            </label>
          </section>

          <section class="group">
            <h3 class="group-title">文件接收设置</h3>
            <div class="field">
              <span>接收文件保存目录</span>
              <div class="dir">
                <input :value="saveDir" readonly placeholder="未设置（使用默认目录）" />
                <button class="btn sm" @click="pickSaveDir">选择文件夹</button>
              </div>
            </div>
            <label class="row">
              <span>同名文件覆盖策略</span>
              <select v-model="collision">
                <option value="rename">自动重命名</option>
                <option value="overwrite">直接覆盖</option>
              </select>
            </label>
          </section>

          <section class="group">
            <h3 class="group-title">设备发现</h3>
            <label class="toggle">
              <span>mDNS 自动发现<small>局域网内自动广播与发现设备</small></span>
              <input v-model="useMdns" type="checkbox" />
            </label>
            <label class="row">
              <span>广播端口</span>
              <input v-model.number="listenPort" type="number" min="1" max="65535" class="num" />
            </label>
          </section>

          <section class="group">
            <h3 class="group-title">本机设备</h3>
            <div class="field">
              <span>设备名称</span>
              <input v-model="deviceName" placeholder="如：我的电脑" />
            </div>
            <label class="toggle">
              <span>隐身模式<small>不被其他设备发现</small></span>
              <input v-model="stealthMode" type="checkbox" />
            </label>
            <label class="toggle">
              <span>已信任设备自动接收<small>配对过的设备发来文件直接接收</small></span>
              <input v-model="autoAcceptTrusted" type="checkbox" />
            </label>
          </section>

          <section class="group">
            <h3 class="group-title">本机信息</h3>
            <div class="info"><span>本机 IP</span><b class="mono">{{ localInfo.ips.join("、") || "—" }}</b></div>
            <div class="info"><span>实际监听端口</span><b class="mono">{{ localInfo.qport || "—" }}</b></div>
            <div class="info fp"><span>本机指纹</span><b class="mono">{{ fingerprint || "—" }}</b><small>配对时与对方屏幕比对</small></div>
          </section>

          <section class="group">
            <h3 class="group-title">维护</h3>
            <div class="danger-zone">
              <button class="btn sm danger" @click="clearRecords">清除传输记录</button>
              <button class="btn sm danger" @click="clearTempCache">清理临时缓存</button>
            </div>
          </section>

          <p class="note">设置保存后立即生效，不影响正在传输中的任务；广播端口变更会在当前任务结束后生效。</p>

          <button class="btn primary block" @click="save">保存设置</button>

          <p class="about">LocalTransfer · v{{ version || "—" }} · 局域网点对点文件传输</p>
        </div>
      </aside>
    </Transition>
  </Teleport>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.35);
  backdrop-filter: blur(2px);
  z-index: 40;
}
.settings-modal {
  position: fixed;
  top: 42px;
  right: 16px;
  bottom: 48px;
  width: min(400px, 94vw);
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-modal);
  box-shadow: var(--shadow-hover), var(--shadow-1);
  z-index: 41;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.settings-modal header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px 10px;
  border-bottom: 1px solid var(--line);
  flex-shrink: 0;
}
.title { margin: 0; font-size: 16px; font-weight: 600; color: var(--text); }
.body {
  padding: 14px 20px 20px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.group { display: flex; flex-direction: column; gap: 10px; }
.group-title {
  margin: 0;
  font-size: 12px;
  font-weight: 600;
  color: var(--accent);
  background: var(--accent-soft);
  border-radius: 8px;
  padding: 5px 10px;
  align-self: flex-start;
}
.group-note { margin: -2px 0 0; font-size: 11px; color: var(--faint); line-height: 1.5; }
.field { display: flex; flex-direction: column; gap: 6px; }
.field > span, .row > span {
  font-size: 13px;
  color: var(--text);
}
.field input, .row input, .row select {
  background: var(--panel-2);
  border: 1px solid var(--line);
  color: var(--text);
  border-radius: var(--r-btn);
  padding: 7px 10px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.12s ease;
}
.field input:focus, .row input:focus, .row select:focus { border-color: var(--accent); }
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.row .num { width: 90px; text-align: right; }
.row select { min-width: 120px; }
.dir { display: flex; gap: 6px; min-width: 0; }
.dir input { flex: 1; min-width: 0; color: var(--muted); font-size: 12px; }
.toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  cursor: pointer;
}
.toggle > span { font-size: 13px; display: flex; flex-direction: column; }
.toggle > span small { color: var(--muted); font-size: 11px; margin-top: 1px; }
.toggle input {
  appearance: none;
  width: 38px;
  height: 22px;
  flex-shrink: 0;
  border-radius: 999px;
  background: #cbd5e1;
  position: relative;
  cursor: pointer;
  transition: background 0.2s ease;
}
.toggle input::after {
  content: "";
  position: absolute;
  top: 3px;
  left: 3px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 3px rgba(15, 23, 42, 0.3);
  transition: left 0.2s ease;
}
.toggle input:checked { background: var(--accent); }
.toggle input:checked::after { left: 19px; }
.info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--muted);
}
.info b {
  font-weight: 400;
  font-size: 12px;
  color: var(--text);
  word-break: break-all;
  background: var(--panel-2);
  border-radius: var(--r-btn);
  padding: 6px 10px;
}
.info.fp small { color: var(--faint); font-size: 11px; }
.mono { font-family: Consolas, monospace; }
.danger-zone { display: flex; gap: 8px; }
.danger-zone .btn { flex: 1; }
.block { width: 100%; padding: 10px; }
.note { margin: 0; font-size: 11px; color: var(--faint); line-height: 1.6; }
.about { margin: 0; text-align: center; font-size: 12px; color: var(--faint); }

/* 遮罩淡入淡出 */
.fade-enter-active, .fade-leave-active { transition: opacity 0.22s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
/* 弹窗自右侧切入 */
.slide-enter-active, .slide-leave-active {
  transition: transform 0.28s cubic-bezier(0.32, 0.72, 0.33, 1), opacity 0.22s ease;
}
.slide-enter-from, .slide-leave-to { transform: translateX(24px); opacity: 0; }
</style>
