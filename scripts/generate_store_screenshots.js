// scripts/generate_store_screenshots.js
// 自动生成符合微软应用商店（Microsoft Store）规范的 4 张 1920x1080 真实应用屏幕截图

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const workspaceDir = path.resolve(__dirname, '..');
const docsScreenshotsDir = path.join(workspaceDir, 'docs', 'screenshots');
const distScreenshotsDir = path.join(workspaceDir, 'dist', 'store_screenshots');
const artifactDir = 'C:\\Users\\geng\\.gemini\\antigravity-ide\\brain\\0e4b7604-0b37-4e27-901b-708e4ae22e27';
const tempDir = path.join(workspaceDir, '.temp_screenshots');

if (!fs.existsSync(docsScreenshotsDir)) fs.mkdirSync(docsScreenshotsDir, { recursive: true });
if (!fs.existsSync(distScreenshotsDir)) fs.mkdirSync(distScreenshotsDir, { recursive: true });
if (!fs.existsSync(tempDir)) fs.mkdirSync(tempDir, { recursive: true });

// 读取 logo base64
const boltLogoBuf = fs.readFileSync(path.join(workspaceDir, 'docs', 'bolt.png'));
const boltLogoBase64 = `data:image/png;base64,${boltLogoBuf.toString('base64')}`;

// 公共 CSS 样式（严格对齐 tauri_app 的现代轻量拟物视觉系统）
const baseCss = `
:root {
  color-scheme: light;
  --bg: #f8fafc;
  --panel: #ffffff;
  --panel-2: #f8fafc;
  --line: #e2e8f0;
  --line-dash: #cbd5e1;
  --text: #1e293b;
  --muted: #64748b;
  --faint: #94a3b8;
  --accent: #2563eb;
  --accent-2: #3b82f6;
  --accent-soft: #eff6ff;
  --ok: #10b981;
  --warn: #f59e0b;
  --danger: #ef4444;
  --shadow-1: 0 8px 24px rgba(0, 0, 0, 0.06), 0 2px 6px rgba(0, 0, 0, 0.04);
  --shadow-2: 0 4px 12px rgba(0, 0, 0, 0.05);
  --shadow-hover: 0 12px 36px rgba(37, 99, 235, 0.12);
  --r-card: 18px;
  --r-item: 12px;
  --r-btn: 10px;
  --r-modal: 20px;
}
* { box-sizing: border-box; }
html, body {
  margin: 0;
  padding: 0;
  width: 1920px;
  height: 1080px;
  overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", "WenQuanYi Micro Hei", "Noto Sans CJK SC", system-ui, sans-serif;
  background: var(--bg);
  color: var(--text);
  user-select: none;
  font-size: 14px;
}
.shell {
  width: 1920px;
  height: 1080px;
  display: flex;
  flex-direction: column;
  background: var(--bg);
}

/* 顶部标题栏 */
.titlebar {
  height: 44px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 0 14px 0 18px;
  background: var(--panel);
  border-bottom: 1px solid var(--line);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
}
.tb-brand { display: flex; align-items: center; gap: 10px; }
.tb-logo { width: 24px; height: 24px; object-fit: contain; }
.tb-brand h1 {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0.3px;
  color: #0f172a;
}
.tb-ver {
  font-size: 11px;
  background: #f1f5f9;
  color: var(--muted);
  padding: 2px 7px;
  border-radius: 6px;
  font-weight: 600;
  margin-left: 2px;
}
.tb-right { display: flex; align-items: center; margin-left: auto; gap: 4px; }
.tb-sep { width: 1px; height: 16px; background: var(--line); margin: 0 8px; }
.tb-btn {
  width: 36px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  border-radius: 7px;
  color: var(--muted);
  cursor: pointer;
}
.tb-btn:hover { background: var(--accent-soft); color: var(--accent); }
.tb-close:hover { background: var(--danger); color: #fff; }

/* 内容两栏布局 (860px + 1fr，视觉更舒展均匀) */
.grid {
  display: grid;
  grid-template-columns: 860px 16px 1fr;
  padding: 16px 20px;
  flex: 1;
  min-height: 0;
  gap: 0;
}
.left-col {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 0;
  height: 100%;
}
.gutter {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
}
.gutter::after {
  content: "";
  width: 4px;
  height: 48px;
  border-radius: 2px;
  background: var(--line-dash);
  opacity: 0.6;
}
.panel {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-card);
  box-shadow: var(--shadow-1);
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
h2 { font-size: 16px; margin: 0; color: #0f172a; font-weight: 700; }
ul { list-style: none; margin: 0; padding: 0; }

/* 按钮规范 */
.btn {
  background: linear-gradient(180deg, #ffffff, #f8fafc);
  border: 1px solid var(--line);
  color: var(--text);
  border-radius: var(--r-btn);
  padding: 8px 16px;
  cursor: pointer;
  font-size: 13.5px;
  font-weight: 500;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.04);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}
.btn.primary {
  background: linear-gradient(135deg, var(--accent-2), var(--accent));
  border-color: transparent;
  color: #fff;
  font-weight: 600;
  box-shadow: 0 4px 14px rgba(37, 99, 235, 0.28);
}
.btn.sm { padding: 5px 12px; font-size: 12.5px; border-radius: 8px; }
.btn.danger { color: var(--danger); border-color: #fecaca; }
.btn.danger:hover { background: #fef2f2; }

.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: transparent;
  border: none;
  color: var(--muted);
  cursor: pointer;
}

/* 底部状态栏 */
.statusbar {
  height: 36px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 0 20px;
  background: var(--panel);
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 12.5px;
  gap: 12px;
}
.sb-item { display: flex; align-items: center; gap: 6px; }
.sb-sep { width: 1px; height: 14px; background: var(--line); }
.sb-val { font-weight: 600; color: var(--text); }
.sb-accent { color: var(--accent); font-weight: 700; }

/* 发送面板 */
.mid-panel {
  flex: 0 0 230px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
}
.mid-panel.has-pending {
  flex: 1 1 0;
}
.dropzone {
  flex: 1;
  border: 2px dashed var(--line-dash);
  border-radius: var(--r-item);
  background: var(--panel-2);
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 16px;
}
.dz-inner { display: flex; flex-direction: column; align-items: center; }
.dz-icon { color: #94a3b8; margin-bottom: 8px; }
.dz-title { margin: 0 0 4px; font-size: 14.5px; font-weight: 600; color: #0f172a; }
.dz-sub { margin: 0 0 12px; font-size: 12px; color: var(--muted); }
.dz-actions { display: flex; gap: 10px; }
.dz-target-hint { margin: 10px 0 0; font-size: 12px; color: var(--accent); font-weight: 500; }

/* 设备面板 */
.dev-panel {
  flex: 1 1 0;
  display: flex;
  flex-direction: column;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.head-title-wrap {
  display: flex;
  align-items: baseline;
  gap: 10px;
}
.wifi-tip {
  font-size: 12px;
  color: var(--faint);
}
.head-actions { display: flex; gap: 4px; }
.devices {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
}
.dev-card {
  display: flex;
  align-items: center;
  gap: 12px;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-item);
  padding: 12px 16px;
  box-shadow: 0 1px 3px rgba(15, 23, 42, 0.04);
}
.dev-card.selected {
  border-color: var(--accent);
  background: var(--accent-soft);
  box-shadow: 0 0 0 1px var(--accent), 0 3px 10px rgba(37, 99, 235, 0.12);
}
.dev-icon {
  width: 40px;
  height: 40px;
  flex-shrink: 0;
  border-radius: 10px;
  background: var(--accent-soft);
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
}
.dev-card.selected .dev-icon { background: #dbeafe; }
.dev-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.dev-name {
  font-weight: 600;
  font-size: 14.5px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.dev-sub {
  color: var(--muted);
  font-size: 12px;
}
.conn-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(16, 185, 129, 0.12);
  color: var(--ok);
}
.conn-badge.idle {
  background: rgba(100, 116, 139, 0.1);
  color: var(--muted);
}

/* 任务面板 */
.task-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
}
.tasks {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
}
.task-item {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-item);
  padding: 14px 16px;
  box-shadow: var(--shadow-2);
  display: flex;
  flex-direction: column;
}
.task-head { display: flex; align-items: center; gap: 10px; }
.t-icon {
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  border-radius: 9px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.t-icon.send { background: var(--accent-soft); color: var(--accent); }
.t-icon.recv { background: rgba(16, 185, 129, 0.12); color: var(--ok); }
.t-name {
  flex: 1;
  min-width: 0;
  font-size: 14px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 6px;
}
.t-dir {
  font-size: 11px;
  font-weight: 400;
  color: var(--muted);
  background: #f1f5f9;
  padding: 1px 6px;
  border-radius: 4px;
}
.t-proto {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--accent);
  background: var(--accent-soft);
  border-radius: 4px;
  padding: 2px 6px;
  letter-spacing: 0.5px;
}
.t-state { font-size: 13px; font-weight: 700; }
.t-state.run { color: var(--accent); }
.t-state.ok { color: var(--ok); }
.task-file {
  color: #334155;
  font-size: 13px;
  font-weight: 500;
  margin-top: 8px;
}
.task-rate {
  color: var(--muted);
  font-size: 12px;
  margin-top: 4px;
  display: flex;
  justify-content: space-between;
}
.bar {
  height: 8px;
  background: #e2e8f0;
  border-radius: 8px;
  overflow: hidden;
  margin-top: 8px;
}
.bar-fill { height: 100%; border-radius: 8px; }
.bar-fill.run { background: linear-gradient(90deg, #60a5fa, var(--accent)); }
.bar-fill.ok { background: var(--ok); }
.task-actions {
  display: flex;
  align-items: center;
  margin-top: 10px;
  font-size: 12px;
  color: var(--muted);
}
.spacer { flex: 1; }
.panel-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 14px;
  padding-top: 12px;
  border-top: 1px solid var(--line);
}
.summary {
  display: flex;
  gap: 16px;
  font-size: 13.5px;
  font-weight: 600;
  color: #334155;
}
`;

// 生成截图 1：局域网设备互联与自动发现
function generateHtml1() {
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>${baseCss}</style>
</head>
<body>
<main class="shell">
  <!-- 标题栏 -->
  <div class="titlebar">
    <div class="tb-brand">
      <img src="${boltLogoBase64}" class="tb-logo" alt="Bolt" />
      <h1>Bolt 闪传</h1>
      <span class="tb-ver">v0.1.0</span>
    </div>
    <div class="tb-right">
      <button class="tb-btn" title="设置">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06-.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
      </button>
      <span class="tb-sep"></span>
      <button class="tb-btn" title="最小化"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14"/></svg></button>
      <button class="tb-btn" title="向下还原"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="8" width="11" height="11" rx="2"/><path d="M9 5h8a2 2 0 0 1 2 2v8"/></svg></button>
      <button class="tb-btn tb-close" title="关闭"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg></button>
    </div>
  </div>

  <!-- 主体网格 -->
  <section class="grid">
    <div class="left-col">
      <!-- 拖拽发送区域 -->
      <div class="panel mid-panel">
        <div class="dropzone">
          <div class="dz-inner">
            <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" class="dz-icon">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <path d="M17 8l-5-5-5 5M12 3v12" />
            </svg>
            <p class="dz-title">将文件或文件夹拖拽到此区域</p>
            <p class="dz-sub">支持单文件、多文件或整个目录批量极速互传</p>
            <div class="dz-actions">
              <button class="btn primary sm dz-btn">发送文件</button>
              <button class="btn sm dz-btn">发送文件夹</button>
            </div>
            <p class="dz-target-hint">已选目标：<b>Xiaomi 14 Ultra (192.168.1.188)</b></p>
          </div>
        </div>
      </div>

      <!-- 设备列表 -->
      <div class="panel dev-panel">
        <div class="panel-head">
          <div class="head-title-wrap">
            <h2>可用设备</h2>
            <span class="wifi-tip">· 当前局域网 (Wi-Fi 6 / 千兆有线)</span>
          </div>
          <div class="head-actions">
            <button class="icon-btn" title="刷新设备"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg></button>
            <button class="icon-btn" title="手动输入 IP"><svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg></button>
          </div>
        </div>

        <ul class="devices">
          <li class="dev-card selected">
            <div class="dev-icon">
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="2" width="14" height="20" rx="2" ry="2"/><line x1="12" y1="18" x2="12.01" y2="18"/></svg>
            </div>
            <div class="dev-info">
              <div class="dev-name">
                Xiaomi 14 Ultra
                <span class="conn-badge">已连接 · QUIC</span>
              </div>
              <div class="dev-sub">192.168.1.188:8899 · Android 14 · 信号极佳</div>
            </div>
            <button class="btn sm">断开</button>
          </li>

          <li class="dev-card">
            <div class="dev-icon">
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
            </div>
            <div class="dev-info">
              <div class="dev-name">
                Ian-Office-Desktop
                <span class="conn-badge idle">在线</span>
              </div>
              <div class="dev-sub">192.168.1.105:8899 · Windows 11 Pro · 千兆有线</div>
            </div>
            <button class="btn sm">连接</button>
          </li>

          <li class="dev-card">
            <div class="dev-icon">
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16"/></svg>
            </div>
            <div class="dev-info">
              <div class="dev-name">
                MacBook-Pro-M3
                <span class="conn-badge idle">在线</span>
              </div>
              <div class="dev-sub">192.168.1.112:8899 · macOS Sequoia · 5GHz 无线</div>
            </div>
            <button class="btn sm">连接</button>
          </li>
        </ul>
      </div>
    </div>

    <!-- 栏宽调整槽 -->
    <div class="gutter"></div>

    <!-- 任务面板 -->
    <div class="panel task-panel">
      <div class="panel-head">
        <h2>传输任务</h2>
        <button class="icon-btn" title="更多操作"><svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg></button>
      </div>

      <ul class="tasks">
        <li class="task-item">
          <div class="task-head">
            <span class="t-icon send"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 19V5M5 12l7-7 7 7"/></svg></span>
            <span class="t-name">Xiaomi 14 Ultra <span class="t-dir">发出</span> <span class="t-proto">QUIC</span></span>
            <span class="t-state ok">完成</span>
          </div>
          <div class="task-file">Design_System_Tokens_2026.fig</div>
          <div class="task-rate">
            <span>24.8 MB · 平均速率 88.4 MB/s</span>
            <span>已成功校验</span>
          </div>
          <div class="bar"><div class="bar-fill ok" style="width: 100%;"></div></div>
          <div class="task-actions">
            <span>传输耗时 0.28 秒 · 1 个文件</span>
            <span class="spacer"></span>
            <button class="btn sm">再次发送</button>
          </div>
        </li>

        <li class="task-item">
          <div class="task-head">
            <span class="t-icon recv"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 5v14M19 12l-7 7-7-7"/></svg></span>
            <span class="t-name">Ian-Office-Desktop <span class="t-dir">接收</span> <span class="t-proto">TCP</span></span>
            <span class="t-state ok">完成</span>
          </div>
          <div class="task-file">Bolt_Release_v0.1.0_Assets.zip</div>
          <div class="task-rate">
            <span>1.42 GB · 平均速率 114.2 MB/s</span>
            <span>BLAKE3 校验通过</span>
          </div>
          <div class="bar"><div class="bar-fill ok" style="width: 100%;"></div></div>
          <div class="task-actions">
            <span>传输耗时 12.4 秒 · 1 个文件</span>
            <span class="spacer"></span>
            <button class="btn sm">打开文件夹</button>
          </div>
        </li>
      </ul>

      <div class="panel-foot">
        <div class="summary">
          <span>总速度 0 MB/s</span>
          <span>剩余 0 秒</span>
        </div>
        <button class="btn sm">清除记录</button>
      </div>
    </div>
  </section>

  <!-- 底部状态栏 -->
  <footer class="statusbar">
    <div class="sb-item">本机：<span class="sb-val">Ian-Studio-PC</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">IP：<span class="sb-val">192.168.1.100</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">在线设备：<span class="sb-val">3 台</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">核心协议：<span class="sb-accent">QUIC / UDP</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">实时速度：<span class="sb-val">0 MB/s</span></div>
  </footer>
</main>
</body>
</html>`;
}

// 生成截图 2：多任务并发极速传输与监控
function generateHtml2() {
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>${baseCss}</style>
</head>
<body>
<main class="shell">
  <!-- 标题栏 -->
  <div class="titlebar">
    <div class="tb-brand">
      <img src="${boltLogoBase64}" class="tb-logo" alt="Bolt" />
      <h1>Bolt 闪传</h1>
      <span class="tb-ver">v0.1.0</span>
    </div>
    <div class="tb-right">
      <button class="tb-btn" title="设置">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06-.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
      </button>
      <span class="tb-sep"></span>
      <button class="tb-btn" title="最小化"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14"/></svg></button>
      <button class="tb-btn" title="向下还原"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="8" width="11" height="11" rx="2"/><path d="M9 5h8a2 2 0 0 1 2 2v8"/></svg></button>
      <button class="tb-btn tb-close" title="关闭"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg></button>
    </div>
  </div>

  <section class="grid">
    <div class="left-col">
      <div class="panel mid-panel">
        <div class="dropzone">
          <div class="dz-inner">
            <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" class="dz-icon">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <path d="M17 8l-5-5-5 5M12 3v12" />
            </svg>
            <p class="dz-title">将文件或文件夹拖拽到此区域</p>
            <p class="dz-sub">多通道并发传输进行中，随时添加新任务</p>
            <div class="dz-actions">
              <button class="btn primary sm dz-btn">发送文件</button>
              <button class="btn sm dz-btn">发送文件夹</button>
            </div>
            <p class="dz-target-hint">当前通道：<b>Xiaomi 14 Ultra (QUIC 协议传输中)</b></p>
          </div>
        </div>
      </div>

      <div class="panel dev-panel">
        <div class="panel-head">
          <div class="head-title-wrap">
            <h2>可用设备</h2>
            <span class="wifi-tip">· 2 个活跃会话</span>
          </div>
          <div class="head-actions">
            <button class="icon-btn"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg></button>
            <button class="icon-btn"><svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg></button>
          </div>
        </div>

        <ul class="devices">
          <li class="dev-card selected">
            <div class="dev-icon">
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="2" width="14" height="20" rx="2" ry="2"/><line x1="12" y1="18" x2="12.01" y2="18"/></svg>
            </div>
            <div class="dev-info">
              <div class="dev-name">
                Xiaomi 14 Ultra
                <span class="conn-badge">传输中 · 98.4 MB/s</span>
              </div>
              <div class="dev-sub">192.168.1.188:8899 · QUIC · 4 线程流</div>
            </div>
            <button class="btn sm">断开</button>
          </li>

          <li class="dev-card">
            <div class="dev-icon">
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
            </div>
            <div class="dev-info">
              <div class="dev-name">
                Ian-Office-Desktop
                <span class="conn-badge">传输中 · 86.8 MB/s</span>
              </div>
              <div class="dev-sub">192.168.1.105:8899 · TCP 异步分块传输</div>
            </div>
            <button class="btn sm">断开</button>
          </li>

          <li class="dev-card">
            <div class="dev-icon">
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16"/></svg>
            </div>
            <div class="dev-info">
              <div class="dev-name">
                MacBook-Pro-M3
                <span class="conn-badge idle">在线</span>
              </div>
              <div class="dev-sub">192.168.1.112:8899 · 空闲待命</div>
            </div>
            <button class="btn sm">连接</button>
          </li>
        </ul>
      </div>
    </div>

    <div class="gutter"></div>

    <!-- 传输任务列表（正在极速狂飙） -->
    <div class="panel task-panel">
      <div class="panel-head">
        <h2>传输任务</h2>
        <button class="icon-btn"><svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg></button>
      </div>

      <ul class="tasks">
        <li class="task-item">
          <div class="task-head">
            <span class="t-icon recv"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 5v14M19 12l-7 7-7-7"/></svg></span>
            <span class="t-name">Xiaomi 14 Ultra <span class="t-dir">接收</span> <span class="t-proto">QUIC</span></span>
            <span class="t-state run">传输中 76%</span>
          </div>
          <div class="task-file">Cinematic_4K_DLogM_60fps.mov</div>
          <div class="task-rate">
            <span>11.2 GB / 14.8 GB · 速率 98.4 MB/s</span>
            <span>剩余约 36 秒</span>
          </div>
          <div class="bar"><div class="bar-fill run" style="width: 76%;"></div></div>
          <div class="task-actions">
            <span>已成功接收 11,480 MB · 0 个错误</span>
            <span class="spacer"></span>
            <button class="btn sm danger">取消</button>
          </div>
        </li>

        <li class="task-item">
          <div class="task-head">
            <span class="t-icon send"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 19V5M5 12l7-7 7 7"/></svg></span>
            <span class="t-name">Ian-Office-Desktop <span class="t-dir">发出</span> <span class="t-proto">TCP</span></span>
            <span class="t-state run">传输中 58%</span>
          </div>
          <div class="task-file">Qwen2.5_7B_Instruct_Q4_K_M.gguf</div>
          <div class="task-rate">
            <span>2.82 GB / 4.86 GB · 速率 86.8 MB/s</span>
            <span>剩余约 24 秒</span>
          </div>
          <div class="bar"><div class="bar-fill run" style="width: 58%;"></div></div>
          <div class="task-actions">
            <span>已传输 2,890 MB · 0 个错误</span>
            <span class="spacer"></span>
            <button class="btn sm danger">取消</button>
          </div>
        </li>

        <li class="task-item">
          <div class="task-head">
            <span class="t-icon recv"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 5v14M19 12l-7 7-7-7"/></svg></span>
            <span class="t-name">MacBook-Pro-M3 <span class="t-dir">接收</span> <span class="t-proto">QUIC</span></span>
            <span class="t-state ok">完成</span>
          </div>
          <div class="task-file">Codebase_Snapshot_v2026.tar.gz</div>
          <div class="task-rate">
            <span>890 MB · 平均速率 106.5 MB/s</span>
            <span>BLAKE3 校验通过</span>
          </div>
          <div class="bar"><div class="bar-fill ok" style="width: 100%;"></div></div>
          <div class="task-actions">
            <span>耗时 8.3 秒 · 1 个文件</span>
            <span class="spacer"></span>
            <button class="btn sm">打开文件夹</button>
          </div>
        </li>
      </ul>

      <div class="panel-foot">
        <div class="summary">
          <span style="color: var(--accent);">总速度 185.2 MB/s</span>
          <span>剩余约 36 秒</span>
        </div>
        <button class="btn sm">全部清除</button>
      </div>
    </div>
  </section>

  <!-- 底部状态栏 -->
  <footer class="statusbar">
    <div class="sb-item">本机：<span class="sb-val">Ian-Studio-PC</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">IP：<span class="sb-val">192.168.1.100</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">在线设备：<span class="sb-val">3 台</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">核心协议：<span class="sb-accent">QUIC 高性能通道</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">实时速度：<span class="sb-accent" style="font-size: 14px;">185.2 MB/s (1.48 Gbps)</span></div>
  </footer>
</main>
</body>
</html>`;
}

// 生成截图 3：文件与文件夹批量拖拽与发送准备清单
function generateHtml3() {
  const pendingCss = `
  .picked-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 6px; }
  .pending-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
  }
  .pend-item {
    display: flex;
    align-items: center;
    gap: 12px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--r-item);
    padding: 10px 14px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.03);
  }
  .p-icon {
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    border-radius: 9px;
    background: var(--accent-soft);
    color: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .p-icon.dir { background: rgba(245, 158, 11, 0.12); color: var(--warn); }
  .p-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .p-name { font-size: 13.5px; font-weight: 600; color: #0f172a; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .p-meta { font-size: 12px; color: var(--muted); }
  .rm {
    background: none;
    border: none;
    color: var(--faint);
    font-size: 18px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
  }
  .rm:hover { color: var(--danger); background: #fef2f2; }
  .send-foot {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }
  .send-target-info {
    text-align: center;
    font-size: 12.5px;
    color: var(--muted);
  }
  .send-main-btn {
    width: 100%;
    padding: 12px;
    font-size: 15px;
    font-weight: 600;
  }
  `;

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>${baseCss}\n${pendingCss}</style>
</head>
<body>
<main class="shell">
  <div class="titlebar">
    <div class="tb-brand">
      <img src="${boltLogoBase64}" class="tb-logo" alt="Bolt" />
      <h1>Bolt 闪传</h1>
      <span class="tb-ver">v0.1.0</span>
    </div>
    <div class="tb-right">
      <button class="tb-btn" title="设置">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06-.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
      </button>
      <span class="tb-sep"></span>
      <button class="tb-btn" title="最小化"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14"/></svg></button>
      <button class="tb-btn" title="向下还原"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="8" width="11" height="11" rx="2"/><path d="M9 5h8a2 2 0 0 1 2 2v8"/></svg></button>
      <button class="tb-btn tb-close" title="关闭"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg></button>
    </div>
  </div>

  <section class="grid">
    <div class="left-col">
      <!-- 处于状态 C：待发送清单 -->
      <div class="panel mid-panel has-pending">
        <div class="picked-head">
          <h2>待发送清单 (4 个项目)</h2>
          <button class="icon-btn" title="清空全部"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg></button>
        </div>

        <ul class="pending-list">
          <li class="pend-item">
            <span class="p-icon dir">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>
            </span>
            <div class="p-info">
              <span class="p-name">Design_Prototypes_2026/</span>
              <span class="p-meta">文件夹 · 递归包含 1,428 个文件 · 1.82 GB</span>
            </div>
            <button class="rm">×</button>
          </li>

          <li class="pend-item">
            <span class="p-icon">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/></svg>
            </span>
            <div class="p-info">
              <span class="p-name">Raw_Camera_DNG_Batch.zip</span>
              <span class="p-meta">文件 · 3.20 GB · 包含 86 张高清无损 RAW</span>
            </div>
            <button class="rm">×</button>
          </li>

          <li class="pend-item">
            <span class="p-icon">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"/><line x1="7" y1="2" x2="7" y2="22"/><line x1="17" y1="2" x2="17" y2="22"/><line x1="2" y1="12" x2="22" y2="12"/></svg>
            </span>
            <div class="p-info">
              <span class="p-name">Product_Keynote_HDR_4K.mp4</span>
              <span class="p-meta">文件 · 1.54 GB · H.265 编码视频</span>
            </div>
            <button class="rm">×</button>
          </li>

          <li class="pend-item">
            <span class="p-icon">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><line x1="10" y1="9" x2="8" y2="9"/></svg>
            </span>
            <div class="p-info">
              <span class="p-name">Technical_Architecture_Spec.pdf</span>
              <span class="p-meta">文件 · 28.4 MB · 核心协议白皮书</span>
            </div>
            <button class="rm">×</button>
          </li>
        </ul>

        <div class="send-foot">
          <div class="send-target-info">
            发送目标：<b>Xiaomi 14 Ultra (192.168.1.188)</b>
          </div>
          <button class="btn primary send-main-btn">
            立即发送（4 项 · 共 6.59 GB）
          </button>
        </div>
      </div>

      <!-- 设备列表（保留但自适应压紧） -->
      <div class="panel dev-panel" style="flex: 0 0 250px;">
        <div class="panel-head">
          <div class="head-title-wrap">
            <h2>可用设备</h2>
            <span class="wifi-tip">· 点击切换发送目标</span>
          </div>
        </div>

        <ul class="devices">
          <li class="dev-card selected">
            <div class="dev-icon">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="2" width="14" height="20" rx="2" ry="2"/><line x1="12" y1="18" x2="12.01" y2="18"/></svg>
            </div>
            <div class="dev-info">
              <div class="dev-name">Xiaomi 14 Ultra <span class="conn-badge">目标</span></div>
              <div class="dev-sub">192.168.1.188:8899 · QUIC 握手就绪</div>
            </div>
            <button class="btn sm">断开</button>
          </li>
          <li class="dev-card">
            <div class="dev-icon">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
            </div>
            <div class="dev-info">
              <div class="dev-name">Ian-Office-Desktop</div>
              <div class="dev-sub">192.168.1.105:8899 · 在线</div>
            </div>
            <button class="btn sm">选择</button>
          </li>
        </ul>
      </div>
    </div>

    <div class="gutter"></div>

    <!-- 任务面板 -->
    <div class="panel task-panel">
      <div class="panel-head">
        <h2>传输任务</h2>
        <button class="icon-btn"><svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg></button>
      </div>

      <ul class="tasks">
        <li class="task-item">
          <div class="task-head">
            <span class="t-icon send"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 19V5M5 12l7-7 7 7"/></svg></span>
            <span class="t-name">Xiaomi 14 Ultra <span class="t-dir">发出</span> <span class="t-proto">QUIC</span></span>
            <span class="t-state ok">完成</span>
          </div>
          <div class="task-file">Bolt_Android_v0.1.0_universal.apk</div>
          <div class="task-rate">
            <span>16.3 MB · 平均速率 92.5 MB/s</span>
            <span>已成功安装校验</span>
          </div>
          <div class="bar"><div class="bar-fill ok" style="width: 100%;"></div></div>
          <div class="task-actions">
            <span>耗时 0.18 秒 · 1 个文件</span>
            <span class="spacer"></span>
            <button class="btn sm">再次发送</button>
          </div>
        </li>
      </ul>

      <div class="panel-foot">
        <div class="summary">
          <span>总速度 0 MB/s</span>
          <span>剩余 0 秒</span>
        </div>
        <button class="btn sm">全部清除</button>
      </div>
    </div>
  </section>

  <!-- 底部状态栏 -->
  <footer class="statusbar">
    <div class="sb-item">本机：<span class="sb-val">Ian-Studio-PC</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">IP：<span class="sb-val">192.168.1.100</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">在线设备：<span class="sb-val">3 台</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">核心协议：<span class="sb-accent">QUIC / UDP</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">待发清单：<span class="sb-val">4 项 (6.59 GB)</span></div>
  </footer>
</main>
</body>
</html>`;
}

// 生成截图 4：专业传输与网络参数偏好设置
function generateHtml4() {
  const settingsModalCss = `
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 23, 42, 0.35);
    backdrop-filter: blur(8px);
    z-index: 40;
  }
  .settings-modal {
    position: fixed;
    top: 36px;
    right: 32px;
    bottom: 36px;
    width: 580px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--r-modal);
    box-shadow: 0 24px 64px rgba(15, 23, 42, 0.22), 0 4px 16px rgba(15, 23, 42, 0.08);
    z-index: 41;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .settings-modal header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 22px;
    border-bottom: 1px solid var(--line);
    background: linear-gradient(180deg, var(--panel), var(--panel-2));
    flex-shrink: 0;
  }
  .header-brand { display: flex; align-items: center; gap: 10px; }
  .header-icon { color: var(--accent); }
  .header-brand h2 { font-size: 17px; font-weight: 700; color: #0f172a; margin: 0; }
  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .module-group { display: flex; flex-direction: column; gap: 6px; }
  .section-header { display: flex; align-items: center; gap: 8px; }
  .accent-bar { width: 3.5px; height: 14px; background: var(--accent); border-radius: 2px; }
  .section-title { font-size: 13.5px; font-weight: 700; color: #0f172a; letter-spacing: 0.3px; }
  .setting-card {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 10px 14px;
    box-shadow: 0 1px 3px rgba(15, 23, 42, 0.03);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .setting-item { display: flex; flex-direction: column; gap: 3px; padding-bottom: 6px; border-bottom: 1px solid #f1f5f9; }
  .setting-item:last-child { border-bottom: none; padding-bottom: 0; }
  .setting-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .setting-title { font-size: 13px; font-weight: 600; color: #1e293b; }
  .setting-desc { font-size: 11px; color: var(--muted); line-height: 1.35; }
  .val-badge {
    background: #f1f5f9;
    border: 1px solid #e2e8f0;
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 11.5px;
    font-family: Consolas, monospace;
    color: #334155;
  }
  .val-badge.accent {
    background: var(--accent-soft);
    border-color: rgba(37, 99, 235, 0.3);
    color: var(--accent);
    font-weight: 600;
  }
  .custom-switch {
    width: 38px;
    height: 22px;
    background: var(--accent);
    border-radius: 11px;
    position: relative;
    cursor: pointer;
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  .custom-switch::after {
    content: "";
    position: absolute;
    top: 2px;
    right: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  }
  .custom-switch.off {
    background: #cbd5e1;
  }
  .custom-switch.off::after {
    left: 2px;
    right: auto;
  }
  .text-input {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 12px;
    width: 170px;
    color: #1e293b;
    background: #f8fafc;
  }
  .footer-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 12px 0 4px;
    border-top: 1px solid var(--line);
    flex-shrink: 0;
  }
  .footer-logo { width: 28px; height: 28px; }
  .app-name { font-size: 14px; font-weight: 700; color: #0f172a; }
  .app-ver { font-size: 11.5px; color: var(--muted); }
  .footer-policy-link { background: none; border: none; color: var(--accent); font-size: 11.5px; cursor: pointer; padding: 0; }
  .app-copyright { font-size: 10.5px; color: var(--faint); }
  `;

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>${baseCss}\n${settingsModalCss}</style>
</head>
<body>
<main class="shell">
  <!-- 底层完整主窗口内容（被半透明高斯模糊覆盖） -->
  <div class="titlebar">
    <div class="tb-brand">
      <img src="${boltLogoBase64}" class="tb-logo" alt="Bolt" />
      <h1>Bolt 闪传</h1>
      <span class="tb-ver">v0.1.0</span>
    </div>
    <div class="tb-right">
      <button class="tb-btn" style="background: var(--accent-soft); color: var(--accent);"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06-.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg></button>
      <span class="tb-sep"></span>
      <button class="tb-btn"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14"/></svg></button>
      <button class="tb-btn"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="8" width="11" height="11" rx="2"/><path d="M9 5h8a2 2 0 0 1 2 2v8"/></svg></button>
      <button class="tb-btn tb-close"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg></button>
    </div>
  </div>

  <section class="grid">
    <div class="left-col">
      <div class="panel mid-panel">
        <div class="dropzone"><div class="dz-inner"><p class="dz-title">将文件或文件夹拖拽到此区域</p><p class="dz-sub">支持单文件、多文件或整个目录批量极速互传</p></div></div>
      </div>
      <div class="panel dev-panel">
        <div class="panel-head"><h2>可用设备</h2></div>
        <ul class="devices">
          <li class="dev-card selected">
            <div class="dev-icon"><svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="2" width="14" height="20" rx="2" ry="2"/></svg></div>
            <div class="dev-info"><div class="dev-name">Xiaomi 14 Ultra <span class="conn-badge">已连接 · QUIC</span></div><div class="dev-sub">192.168.1.188:8899</div></div>
          </li>
          <li class="dev-card">
            <div class="dev-icon"><svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/></svg></div>
            <div class="dev-info"><div class="dev-name">Ian-Office-Desktop</div><div class="dev-sub">192.168.1.105:8899</div></div>
          </li>
        </ul>
      </div>
    </div>
    <div class="gutter"></div>
    <div class="panel task-panel">
      <div class="panel-head"><h2>传输任务</h2></div>
      <ul class="tasks">
        <li class="task-item">
          <div class="task-head">
            <span class="t-icon send"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 19V5M5 12l7-7 7 7"/></svg></span>
            <span class="t-name">Xiaomi 14 Ultra <span class="t-dir">发出</span> <span class="t-proto">QUIC</span></span>
            <span class="t-state ok">完成</span>
          </div>
          <div class="task-file">Design_System_Tokens_2026.fig</div>
          <div class="bar"><div class="bar-fill ok" style="width: 100%;"></div></div>
        </li>
      </ul>
    </div>
  </section>

  <footer class="statusbar">
    <div class="sb-item">本机：<span class="sb-val">Ian-Studio-PC</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">IP：<span class="sb-val">192.168.1.100</span></div>
    <span class="sb-sep"></span>
    <div class="sb-item">在线设备：<span class="sb-val">3 台</span></div>
  </footer>

  <!-- 半透明高斯模糊遮罩蒙层 -->
  <div class="backdrop"></div>

  <!-- 高质感立体设置抽屉弹窗 -->
  <aside class="settings-modal">
    <header>
      <div class="header-brand">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="header-icon"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06-.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
        <h2>设置</h2>
      </div>
      <button class="icon-btn" title="关闭 (Esc)"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg></button>
    </header>

    <div class="modal-body">
      <!-- 1. 本机设备 -->
      <section class="module-group">
        <div class="section-header">
          <div class="accent-bar"></div>
          <span class="section-title">本机设备</span>
        </div>
        <div class="setting-card">
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">设备名称</span>
              <input class="text-input" value="Ian-Studio-PC" />
            </div>
            <div class="setting-desc">局域网内其他设备发现与展示的名称</div>
          </div>
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">本机网络 IP</span>
              <div class="val-badge accent">192.168.1.100 · 千兆以太网</div>
            </div>
            <div class="setting-desc">当前活跃的物理网络地址（支持多网卡与热点）</div>
          </div>
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">关闭时最小化到托盘</span>
              <div class="custom-switch"></div>
            </div>
            <div class="setting-desc">关闭主窗口后常驻后台静默接收传输任务</div>
          </div>
        </div>
      </section>

      <!-- 2. 网络传输 -->
      <section class="module-group">
        <div class="section-header">
          <div class="accent-bar"></div>
          <span class="section-title">网络与传输协议</span>
        </div>
        <div class="setting-card">
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">优先 QUIC 传输协议</span>
              <div class="custom-switch"></div>
            </div>
            <div class="setting-desc">基于 UDP 的多路复用高性能传输，抗丢包弱网极速首选</div>
          </div>
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">并发传输流数</span>
              <div class="val-badge">4 个并行流</div>
            </div>
            <div class="setting-desc">大文件传输并发工作流数，充分压榨局域网全双工带宽</div>
          </div>
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">数据块切片大小</span>
              <div class="val-badge">1024 KB (1 MB)</div>
            </div>
            <div class="setting-desc">底层分块流水线吞吐粒度</div>
          </div>
        </div>
      </section>

      <!-- 3. 文件接收 -->
      <section class="module-group">
        <div class="section-header">
          <div class="accent-bar"></div>
          <span class="section-title">文件接收与存储</span>
        </div>
        <div class="setting-card">
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">默认保存路径</span>
              <div class="val-badge" style="max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">D:\\Downloads\\Bolt</div>
            </div>
            <div class="setting-desc">接收文件的默认落盘目录</div>
          </div>
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">同名文件策略</span>
              <div class="val-badge">自动重命名 (推荐)</div>
            </div>
            <div class="setting-desc">目标文件存在时自动添加序号，安全防覆盖</div>
          </div>
        </div>
      </section>

      <!-- 4. 安全与发现 -->
      <section class="module-group">
        <div class="section-header">
          <div class="accent-bar"></div>
          <span class="section-title">设备安全与核心</span>
        </div>
        <div class="setting-card">
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">设备安全指纹</span>
              <div class="val-badge accent">8f4a-92bc-3e11-d078</div>
            </div>
            <div class="setting-desc">用于设备防伪校验的 BLAKE3 安全指纹标识</div>
          </div>
          <div class="setting-item">
            <div class="setting-row">
              <span class="setting-title">传输引擎架构</span>
              <div class="val-badge">Bolt Rust Core (v0.1.0)</div>
            </div>
            <div class="setting-desc">纯本地局域网点对点、零云端上传、零隐私追踪承诺</div>
          </div>
        </div>
      </section>

      <!-- 底部版权 -->
      <div class="footer-info">
        <img src="${boltLogoBase64}" class="footer-logo" />
        <span class="app-name">Bolt 闪传</span>
        <span class="app-ver">版本 v0.1.0 (Windows x64)</span>
        <button class="footer-policy-link">查看《用户隐私政策与安全白皮书》</button>
        <span class="app-copyright">© 2026 Ian Geng. All Rights Reserved.</span>
      </div>
    </div>
  </aside>
</main>
</body>
</html>`;
}

// 写入 4 个 HTML 并执行渲染
const screens = [
  { id: 1, name: 'screenshot_1_discovery', title: '局域网设备互联与自动发现', gen: generateHtml1 },
  { id: 2, name: 'screenshot_2_transfer', title: '多任务并发极速传输与监控', gen: generateHtml2 },
  { id: 3, name: 'screenshot_3_batch_send', title: '文件与文件夹批量拖拽与发送清单', gen: generateHtml3 },
  { id: 4, name: 'screenshot_4_settings', title: '专业传输参数与核心配置', gen: generateHtml4 },
];

const edgePath = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';

for (const s of screens) {
  const htmlContent = s.gen();
  const htmlPath = path.join(tempDir, `${s.name}.html`);
  const docsPngPath = path.join(docsScreenshotsDir, `${s.name}.png`);
  const distPngPath = path.join(distScreenshotsDir, `${s.name}.png`);
  const artifactPngPath = path.join(artifactDir, `${s.name}.png`);

  fs.writeFileSync(htmlPath, htmlContent, 'utf8');

  console.log(`[Rendering] ${s.title} (${s.name})...`);
  execFileSync(edgePath, [
    '--headless',
    '--disable-gpu',
    '--force-device-scale-factor=1',
    `--screenshot=${docsPngPath}`,
    '--window-size=1920,1080',
    htmlPath
  ], { stdio: 'inherit' });

  // 同步复制到 dist/store_screenshots 和 artifactDir
  fs.copyFileSync(docsPngPath, distPngPath);
  try {
    fs.copyFileSync(docsPngPath, artifactPngPath);
  } catch (e) {
    console.warn('Could not copy to artifact dir:', e.message);
  }

  console.log(`[Success] Saved to ${docsPngPath} (1920x1080)`);
}

console.log('\nAll 4 Microsoft Store screenshots have been generated successfully!');
