<script setup lang="ts">
// 设置弹窗（高质感立体重排版 · 即改即生效）：
// 1. 六大核心模块：本机设备、网络传输、文件接收、设备发现、存储与维护、关于本机。
// 2. 交互对齐 Android 端：所有设置项变更后自动即时持久化（即改即生效），完全无需底端“保存设置”按钮。
// 3. 行内对齐：第一行容纳参数名（左）与具体的参数值/控件（右对齐）；第二行另起一行展示补充描述。
// 4. 参数值自适应折行完整展示，严禁截断省略。
// 5. 设备安全指纹默认隐藏，带小眼睛切换明文与密文，支持一键复制到剪贴板。
// 6. 立体科技视觉：微投影卡片、高亮垂直色标、精致 Badge。
import { onMounted, onUnmounted, ref, watch } from "vue";
import { open as pickDialog } from "@tauri-apps/plugin-dialog";
import { useLt } from "../composables/useLt";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "update:open", v: boolean): void }>();

const {
  fingerprint,
  localInfo,
  localIpsText,
  version,
  loadConfig,
  saveConfig,
  clearRecords,
  clearTempCache,
  showToast,
  refreshLocalInfo,
} = useLt();

const loaded = ref(false);

// 1. 本机设备
const deviceName = ref("");
const stealthMode = ref(false);
const minimizeToTray = ref(false);

// 2. 网络传输
const preferQuic = ref(true);
const concurrency = ref(4);
const chunkSize = ref(1024 * 1024);

// 3. 文件接收
const saveDir = ref("");
const collision = ref("rename");
const autoAcceptTrusted = ref(false);

// 4. 设备发现
const useMdns = ref(true);
const listenPort = ref(8899);

// 6. 安全指纹显隐
const fingerprintVisible = ref(false);

function close() {
  emit("update:open", false);
}

watch(
  () => props.open,
  async (v) => {
    if (v) {
      void refreshLocalInfo();
      if (!loaded.value) {
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
        minimizeToTray.value = Boolean(cfg.minimize_to_tray);
        autoAcceptTrusted.value = Boolean(cfg.auto_accept_trusted);
        loaded.value = true;
      }
    }
  },
);

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && props.open) close();
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));

// 即改即生效：修改配置项即时异步持久化
async function updateConfig(patch: Record<string, unknown>, silent = true) {
  await saveConfig(patch, silent);
}

async function onNameChange() {
  const name = deviceName.value.trim();
  if (name) {
    await updateConfig({ device_name: name });
  }
}

async function onPortChange() {
  const p = Math.round(Number(listenPort.value));
  if (p >= 1 && p <= 65535) {
    listenPort.value = p;
    await updateConfig({ listen_port: p });
  } else {
    listenPort.value = 8899;
    await updateConfig({ listen_port: 8899 });
  }
}

async function pickSaveDir() {
  const picked = await pickDialog({ directory: true, title: "选择接收文件保存目录" });
  if (picked && !Array.isArray(picked)) {
    let clean = picked.replace(/[/\\]+$/, "");
    if (!/[/\\]LocalTransfer$/i.test(clean)) {
      clean = clean + "\\LocalTransfer";
    }
    saveDir.value = clean;
    await updateConfig({ save_dir: clean });
    showToast(`接收目录已更新为：${clean}`);
  }
}

async function copyFingerprint() {
  if (fingerprint.value) {
    try {
      await navigator.clipboard.writeText(fingerprint.value);
      showToast("设备指纹已复制到剪贴板");
    } catch {
      showToast("复制失败，请手动选择复制");
    }
  }
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
          <div class="header-brand">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor"
              stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="header-icon">
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
              <circle cx="12" cy="12" r="3"/>
            </svg>
            <h2 class="title">设置</h2>
          </div>
          <button class="icon-btn" title="关闭 (Esc)" aria-label="关闭" @click="close">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
              stroke-width="2" stroke-linecap="round">
              <path d="M18 6 6 18M6 6l12 12" />
            </svg>
          </button>
        </header>

        <div class="body">
          <!-- 1. 本机设备 -->
          <section class="module-group">
            <div class="section-header">
              <div class="accent-bar"></div>
              <span class="section-title">本机设备</span>
            </div>
            <div class="setting-card">
              <!-- 设备名称 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">设备名称</span>
                  <div class="setting-ctrl">
                    <input
                      v-model="deviceName"
                      class="text-input"
                      placeholder="如：我的电脑"
                      @change="onNameChange"
                      @keydown.enter="($event.target as HTMLElement)?.blur()"
                    />
                  </div>
                </div>
                <div class="setting-desc">局域网内其他设备发现与展示的名称</div>
              </div>

              <!-- 本机 IP -->
              <div class="setting-item">
                <div class="setting-row" :class="{ 'align-top': localInfo.ips.length > 1 }">
                  <span class="setting-title" :class="{ 'title-pad': localInfo.ips.length > 1 }">本机 IP</span>
                  <div class="setting-ctrl">
                    <div class="val-badge mono">
                      {{ localIpsText }}
                    </div>
                  </div>
                </div>
                <div class="setting-desc">当前连接的局域网物理网络地址（支持多网卡/热点）</div>
              </div>

              <!-- 实际监听端口 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">实际监听端口</span>
                  <div class="setting-ctrl">
                    <div class="val-badge mono">
                      {{ localInfo.qport || "—" }}
                    </div>
                  </div>
                </div>
                <div class="setting-desc">传输核心实际绑定的本地监听端口</div>
              </div>

              <!-- 隐身模式 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">隐身模式</span>
                  <div class="setting-ctrl">
                    <input
                      v-model="stealthMode"
                      type="checkbox"
                      class="custom-switch"
                      @change="updateConfig({ stealth_mode: stealthMode })"
                    />
                  </div>
                </div>
                <div class="setting-desc">不在对方设备列表中广播本机，仍可被手动输入 IP 连接</div>
              </div>

              <!-- 关闭时最小化到托盘 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">关闭时最小化到托盘</span>
                  <div class="setting-ctrl">
                    <input
                      v-model="minimizeToTray"
                      type="checkbox"
                      class="custom-switch"
                      @change="updateConfig({ minimize_to_tray: minimizeToTray })"
                    />
                  </div>
                </div>
                <div class="setting-desc">点击关闭按钮时隐藏到系统托盘，保持后台运行而非退出</div>
              </div>
            </div>
          </section>

          <!-- 2. 网络传输 -->
          <section class="module-group">
            <div class="section-header">
              <div class="accent-bar"></div>
              <span class="section-title">网络传输</span>
            </div>
            <div class="setting-card">
              <!-- 传输协议 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">传输协议</span>
                  <div class="setting-ctrl">
                    <select
                      v-model="preferQuic"
                      class="custom-select"
                      @change="updateConfig({ prefer_quic: preferQuic })"
                    >
                      <option :value="true">QUIC（推荐）</option>
                      <option :value="false">TCP</option>
                    </select>
                  </div>
                </div>
                <div class="setting-desc">两端任一设备选择 TCP 即使用 TCP，均选 QUIC 才用 QUIC</div>
              </div>

              <!-- 并发流数量 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">并发流数量</span>
                  <div class="setting-ctrl">
                    <select
                      v-model.number="concurrency"
                      class="custom-select"
                      @change="updateConfig({ concurrency: concurrency })"
                    >
                      <option :value="1">1 个流</option>
                      <option :value="2">2 个流</option>
                      <option :value="4">4 个并发流（推荐）</option>
                      <option :value="8">8 个并发流</option>
                      <option :value="12">12 个并发流</option>
                      <option :value="16">16 个并发流</option>
                    </select>
                  </div>
                </div>
                <div class="setting-desc">批量文件传输时的并行流数量（推荐 4）</div>
              </div>

              <!-- 分片大小 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">分片大小</span>
                  <div class="setting-ctrl">
                    <select
                      v-model.number="chunkSize"
                      class="custom-select"
                      @change="updateConfig({ chunk_size: chunkSize })"
                    >
                      <option :value="256 * 1024">256KB</option>
                      <option :value="512 * 1024">512KB</option>
                      <option :value="1024 * 1024">1MB（推荐）</option>
                      <option :value="4 * 1024 * 1024">4MB</option>
                    </select>
                  </div>
                </div>
                <div class="setting-desc">单次传输拆包尺寸，影响吞吐与内存负载</div>
              </div>
            </div>
          </section>

          <!-- 3. 文件接收 -->
          <section class="module-group">
            <div class="section-header">
              <div class="accent-bar"></div>
              <span class="section-title">文件接收</span>
            </div>
            <div class="setting-card">
              <!-- 保存目录 -->
              <div class="setting-item">
                <div class="setting-row align-top">
                  <span class="setting-title title-pad">保存目录</span>
                  <div class="setting-ctrl">
                    <div
                      class="val-badge mono accent clickable"
                      title="点击更改保存目录"
                      @click="pickSaveDir"
                    >
                      <span class="path-text">{{ saveDir || "未设置（使用默认目录）" }}</span>
                      <span class="badge-tag">📁 更改</span>
                    </div>
                  </div>
                </div>
                <div class="setting-desc">选择的文件夹为父目录，末级目录自动固定为 /LocalTransfer</div>
              </div>

              <!-- 同名冲突策略 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">同名冲突策略</span>
                  <div class="setting-ctrl">
                    <select
                      v-model="collision"
                      class="custom-select"
                      @change="updateConfig({ collision: collision })"
                    >
                      <option value="rename">自动重命名</option>
                      <option value="overwrite">直接覆盖</option>
                    </select>
                  </div>
                </div>
                <div class="setting-desc">目标目录中已存在同名文件时的应对方式</div>
              </div>

              <!-- 自动接收信任文件 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">自动接收信任文件</span>
                  <div class="setting-ctrl">
                    <input
                      v-model="autoAcceptTrusted"
                      type="checkbox"
                      class="custom-switch"
                      @change="updateConfig({ auto_accept_trusted: autoAcceptTrusted })"
                    />
                  </div>
                </div>
                <div class="setting-desc">已完成配对的信任设备发送文件时，免确认直接开始接收</div>
              </div>
            </div>
          </section>

          <!-- 4. 设备发现 -->
          <section class="module-group">
            <div class="section-header">
              <div class="accent-bar"></div>
              <span class="section-title">设备发现</span>
            </div>
            <div class="setting-card">
              <!-- mDNS 自动发现 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">mDNS 自动发现</span>
                  <div class="setting-ctrl">
                    <input
                      v-model="useMdns"
                      type="checkbox"
                      class="custom-switch"
                      @change="updateConfig({ use_mdns: useMdns })"
                    />
                  </div>
                </div>
                <div class="setting-desc">局域网内通过多播 DNS 自动广播与发现设备</div>
              </div>

              <!-- 广播探测端口 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">广播探测端口</span>
                  <div class="setting-ctrl">
                    <input
                      v-model.number="listenPort"
                      type="number"
                      min="1"
                      max="65535"
                      class="num-input"
                      @change="onPortChange"
                      @keydown.enter="($event.target as HTMLElement)?.blur()"
                    />
                  </div>
                </div>
                <div class="setting-desc">UDP 广播探测端口，修改后在当前传输任务全部结束后生效</div>
              </div>
            </div>
          </section>

          <!-- 5. 存储与维护 -->
          <section class="module-group">
            <div class="section-header">
              <div class="accent-bar"></div>
              <span class="section-title">存储与维护</span>
            </div>
            <div class="setting-card">
              <!-- 传输任务记录 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">传输任务记录</span>
                  <div class="setting-ctrl">
                    <button class="btn sm danger-outline" @click="clearRecords">全部清除</button>
                  </div>
                </div>
                <div class="setting-desc">清理列表中所有已完成、失败或取消的历史记录</div>
              </div>

              <!-- 传输临时缓存 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">传输临时缓存</span>
                  <div class="setting-ctrl">
                    <button class="btn sm danger-outline" @click="clearTempCache">清理缓存</button>
                  </div>
                </div>
                <div class="setting-desc">删除未完成的接收碎片与断点续传临时缓存文件</div>
              </div>
            </div>
          </section>

          <!-- 6. 关于本机 -->
          <section class="module-group">
            <div class="section-header">
              <div class="accent-bar"></div>
              <span class="section-title">关于本机</span>
            </div>
            <div class="setting-card">
              <!-- 设备安全指纹 -->
              <div class="setting-item">
                <div class="setting-row" :class="{ 'align-top': fingerprintVisible }">
                  <span class="setting-title" :class="{ 'title-pad': fingerprintVisible }">设备安全指纹</span>
                  <div class="setting-ctrl fp-ctrl">
                    <div
                      class="val-badge mono clickable"
                      :class="{ accent: fingerprintVisible }"
                      :title="fingerprint ? '点击复制安全指纹' : ''"
                      @click="copyFingerprint"
                    >
                      <span class="fp-text">{{ fingerprintVisible ? (fingerprint || "—") : "••••••••••••" }}</span>
                    </div>
                    <button
                      class="eye-btn"
                      :class="{ active: fingerprintVisible }"
                      :title="fingerprintVisible ? '隐藏安全指纹' : '显示安全指纹'"
                      @click="fingerprintVisible = !fingerprintVisible"
                    >
                      <svg v-if="fingerprintVisible" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                        stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M9.88 9.88a3 3 0 1 0 4.24 4.24m-6.72-2.12a10.96 10.96 0 0 1-5.4-2 10.94 10.94 0 0 1 18.72-3.13m-2.12 4.25a10.94 10.94 0 0 1-2.6 3.88M3 3l18 18" />
                      </svg>
                      <svg v-else width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                        stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" />
                        <circle cx="12" cy="12" r="3" />
                      </svg>
                    </button>
                  </div>
                </div>
                <div class="setting-desc">用于身份配对校验的 BLAKE3 安全标识（点击复制，默认隐藏）</div>
              </div>

              <!-- 传输引擎版本 -->
              <div class="setting-item">
                <div class="setting-row">
                  <span class="setting-title">传输引擎版本</span>
                  <div class="setting-ctrl">
                    <div class="val-badge mono">
                      {{ version || "0.1.0" }}
                    </div>
                  </div>
                </div>
                <div class="setting-desc">LocalTransfer Rust P2P Core 核心底层引擎</div>
              </div>
            </div>
          </section>

          <!-- 底部版本信息 -->
          <div class="footer-info">
            <p class="about">LocalTransfer · v{{ version || "0.1.0" }} · 局域网点对点文件传输</p>
          </div>
        </div>
      </aside>
    </Transition>
  </Teleport>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(4px);
  z-index: 40;
}
.settings-modal {
  position: fixed;
  top: 48px;
  right: 18px;
  bottom: 40px;
  width: min(480px, 94vw);
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-modal);
  box-shadow: 0 20px 48px rgba(15, 23, 42, 0.16), 0 4px 12px rgba(15, 23, 42, 0.08);
  z-index: 41;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.settings-modal header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px 14px;
  border-bottom: 1px solid var(--line);
  background: linear-gradient(180deg, var(--panel), var(--panel-2));
  flex-shrink: 0;
}
.header-brand {
  display: flex;
  align-items: center;
  gap: 8px;
}
.header-icon {
  color: var(--accent);
}
.title {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: var(--text);
  letter-spacing: 0.3px;
}

.body {
  padding: 16px 20px 24px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

/* 模块容器与立体标题条 */
.module-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.section-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-left: 2px;
}
.accent-bar {
  width: 3.5px;
  height: 14px;
  background: var(--accent);
  border-radius: 2px;
}
.section-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--accent);
  letter-spacing: 0.3px;
}

/* 立体卡片容器 */
.setting-card {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 14px;
  box-shadow: 0 2px 8px rgba(15, 23, 42, 0.04), 0 1px 2px rgba(15, 23, 42, 0.02);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: box-shadow 0.2s ease, border-color 0.2s ease;
}
.setting-card:hover {
  box-shadow: 0 4px 14px rgba(15, 23, 42, 0.06), 0 1px 3px rgba(15, 23, 42, 0.03);
}

/* 行布局：第一行左右对齐，第二行独立描述 */
.setting-item {
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  border-bottom: 1px solid rgba(226, 232, 240, 0.65);
}
.setting-item:last-child {
  border-bottom: none;
}
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
}
.setting-row.align-top {
  align-items: flex-start;
}
.setting-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  flex-shrink: 0;
}
.setting-title.title-pad {
  padding-top: 4px;
}
.setting-desc {
  font-size: 11px;
  color: var(--faint);
  line-height: 1.5;
  margin-top: 4px;
}

/* 控件区靠右对齐 */
.setting-ctrl {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  flex: 1;
  min-width: 0;
}
.fp-ctrl {
  max-width: 75%;
}

/* 参数值 Badge：立体制感、超宽自动折行不截断 */
.val-badge {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
  padding: 5px 10px;
  border-radius: 8px;
  background: var(--panel-2);
  border: 1px solid var(--line);
  font-size: 12px;
  color: var(--text);
  text-align: right;
  word-break: break-all;
  white-space: normal;
  line-height: 1.45;
  max-width: 100%;
  transition: all 0.15s ease;
}
.val-badge.clickable {
  cursor: pointer;
  user-select: text;
}
.val-badge.clickable:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.val-badge.accent {
  color: var(--accent);
  background: var(--accent-soft);
  border-color: rgba(37, 99, 235, 0.25);
  font-weight: 500;
}
.val-badge.mono {
  font-family: Consolas, "JetBrains Mono", monospace;
}
.badge-tag {
  font-size: 11px;
  font-family: system-ui, sans-serif;
  color: var(--accent);
  background: #ffffff;
  border-radius: 6px;
  padding: 2px 6px;
  box-shadow: 0 1px 2px rgba(37, 99, 235, 0.12);
  flex-shrink: 0;
}
.fp-text, .path-text {
  word-break: break-all;
  line-height: 1.4;
}

/* 小眼睛切换按钮 */
.eye-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--panel-2);
  border: 1px solid var(--line);
  color: var(--muted);
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.15s ease;
}
.eye-btn:hover {
  color: var(--accent);
  background: var(--accent-soft);
  border-color: rgba(37, 99, 235, 0.35);
}
.eye-btn.active {
  color: var(--accent);
  background: var(--accent-soft);
  border-color: var(--accent);
}

/* 输入框与下拉选择框 */
.text-input, .num-input {
  background: var(--panel-2);
  border: 1px solid var(--line);
  border-radius: var(--r-btn);
  padding: 6px 10px;
  font-size: 12.5px;
  color: var(--text);
  outline: none;
  text-align: right;
  transition: all 0.15s ease;
}
.text-input {
  width: 170px;
  max-width: 100%;
}
.num-input {
  width: 86px;
}
.text-input:focus, .num-input:focus {
  border-color: var(--accent);
  background: #ffffff;
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.custom-select {
  appearance: none;
  background: var(--panel-2) url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2364748b' stroke-width='2.2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolyline points='6 9 12 15 18 9'%3E%3C/polyline%3E%3C/svg%3E") no-repeat right 9px center;
  border: 1px solid var(--line);
  border-radius: var(--r-btn);
  padding: 6px 28px 6px 12px;
  font-size: 12.5px;
  color: var(--text);
  font-weight: 500;
  cursor: pointer;
  outline: none;
  transition: all 0.15s ease;
}
.custom-select:hover, .custom-select:focus {
  border-color: var(--accent);
  background-color: #ffffff;
  box-shadow: 0 0 0 3px var(--accent-soft);
}

/* Switch 开关组件 */
.custom-switch {
  appearance: none;
  width: 40px;
  height: 22px;
  flex-shrink: 0;
  border-radius: 999px;
  background: #cbd5e1;
  position: relative;
  cursor: pointer;
  outline: none;
  border: none;
  transition: background 0.2s ease, box-shadow 0.2s ease;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.1);
}
.custom-switch::after {
  content: "";
  position: absolute;
  top: 2.5px;
  left: 3px;
  width: 17px;
  height: 17px;
  border-radius: 50%;
  background: #ffffff;
  box-shadow: 0 1px 3px rgba(15, 23, 42, 0.3);
  transition: transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.custom-switch:checked {
  background: var(--accent);
}
.custom-switch:checked::after {
  transform: translateX(17px);
}

/* 危险操作按钮（立体边框） */
.danger-outline {
  background: var(--panel);
  color: var(--danger);
  border: 1px solid #fecaca;
  border-radius: 8px;
  padding: 4px 12px;
  font-size: 12px;
  font-weight: 500;
  box-shadow: 0 1px 2px rgba(239, 68, 68, 0.05);
  transition: all 0.15s ease;
}
.danger-outline:hover {
  background: #fef2f2;
  border-color: #fca5a5;
  transform: translateY(-1px);
  box-shadow: 0 3px 8px rgba(239, 68, 68, 0.12);
}
.danger-outline:active {
  transform: translateY(0);
}

/* 底部区域 */
.footer-info {
  display: flex;
  justify-content: center;
  padding-top: 6px;
  padding-bottom: 2px;
}
.about {
  margin: 0;
  text-align: center;
  font-size: 12px;
  color: var(--faint);
}

/* 遮罩淡入淡出与滑入 */
.fade-enter-active, .fade-leave-active { transition: opacity 0.22s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
.slide-enter-active, .slide-leave-active {
  transition: transform 0.28s cubic-bezier(0.32, 0.72, 0.33, 1), opacity 0.22s ease;
}
.slide-enter-from, .slide-leave-to { transform: translateX(24px); opacity: 0; }
</style>
