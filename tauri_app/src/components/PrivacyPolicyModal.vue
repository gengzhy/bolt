<script setup lang="ts">
defineProps<{
  show: boolean;
}>();

defineEmits<{
  (e: "close"): void;
}>();
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="show" class="policy-mask" @click.self="$emit('close')">
        <div class="policy-dialog">
          <!-- 头部 -->
          <div class="policy-header">
            <div class="title-wrap">
              <div class="shield-icon">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                  <path d="m9 12 2 2 4-4"/>
                </svg>
              </div>
              <div>
                <h3 class="dialog-title">Bolt 隐私政策与数据安全声明</h3>
                <p class="dialog-sub">纯局域网传输 · 零云端上传 · 零第三方追踪 · TLS 1.3 强制加密</p>
              </div>
            </div>
            <button class="close-btn" title="关闭" @click="$emit('close')">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M18 6 6 18M6 6l12 12"/>
              </svg>
            </button>
          </div>

          <!-- 正文可滚动区域 -->
          <div class="policy-content">
            <div class="notice-box">
              <span class="notice-tag">核心承诺</span>
              <p>Bolt 是一款开源、纯局域网点对点（P2P）文件传输工具。我们没有中心服务器，不收集、不上报、不存储任何个人隐私文件。所有传输仅在您局域网授权的两台设备之间直接直连。</p>
            </div>

            <section class="section">
              <h4>1. 核心隐私承诺</h4>
              <ul>
                <li><strong>零服务器与零云端</strong>：无用户注册系统，无远程中继服务器，从物理架构上杜绝外网数据泄露风险。</li>
                <li><strong>纯局域网直连</strong>：文件流直接由发送设备发送至接收设备，全程不经过任何外部互联网节点。</li>
                <li><strong>零第三方追踪</strong>：不包含任何商业广告 SDK、行为分析或遥测追踪代码。</li>
                <li><strong>端到端加密</strong>：传输链路全程强制 TLS 1.3 加密与 BLAKE3 全流程校验，防范局域网窃听与篡改。</li>
              </ul>
            </section>

            <section class="section">
              <h4>2. 我们如何处理设备信息与数据</h4>
              <p>仅在满足局域网互联互通所必需的范围内，在您本地设备与对端设备之间处理以下数据：</p>
              <ul>
                <li><strong>设备识别信息</strong>：设备自定义昵称、操作系统标识、局域网内网 IP 与端口，仅用于局域网 mDNS/NSD 自动发现与对端寻址。</li>
                <li><strong>安全证书指纹</strong>：本地生成的 Ed25519 自签名公钥哈希，用于生成 4 位动态配对验证码及首次连接信任认证（TOFU），不含硬件隐私。</li>
                <li><strong>传输文件数据</strong>：文件元数据及内容数据仅向您明确指定的对端设备发送。传输中接收端暂存为 <code>.bttmp</code> 临时文件，校验完成即重命名。</li>
                <li><strong>本地配置存储</strong>：设备昵称、下载目录、信任设备列表均仅存储于本地，应用卸载即彻底删除。</li>
              </ul>
            </section>

            <section class="section">
              <h4>3. 操作系统权限使用说明</h4>
              <ul>
                <li><strong>网络与局域网通信</strong>：用于监听传输端口与建立局域网 TCP/QUIC P2P 传输通道。首次启动请允许防火墙专用网络访问。</li>
                <li><strong>文件读写访问</strong>：用于读取您主动选择发送的文件，以及将接收文件保存至您指定的下载目录。</li>
              </ul>
            </section>

            <section class="section">
              <h4>4. 数据安全保障机制</h4>
              <ul>
                <li><strong>TLS 1.3 强制加密</strong>：通信握手后全流量加密传输，不支持明文回退。</li>
                <li><strong>TOFU 首次信任机制</strong>：核验 4 位验证码后信任对端，若对端证书异常变更将即刻报警阻断。</li>
                <li><strong>BLAKE3 数据完整性校验</strong>：传输完成后执行全文件哈希核对，确保字节级一致无损。</li>
              </ul>
            </section>

            <section class="section">
              <h4>5. 用户权利与数据自主权</h4>
              <ul>
                <li><strong>管理与删除信任设备</strong>：您可在设置中随时删除已信任设备，重置配对关系。</li>
                <li><strong>自定义保存位置与设备名称</strong>：完全自由掌控对外显示的名称及文件保存路径。</li>
                <li><strong>彻底销毁数据</strong>：应用卸载或删除本地应用数据目录即可完全销毁所有本地配置与缓存。</li>
              </ul>
            </section>

            <section class="section">
              <h4>6. 联系与开源社区</h4>
              <p>Bolt 为开源软件项目，欢迎在代码仓库审查所有源代码。若有疑问或建议，欢迎联系：</p>
              <p class="contact-line">电子邮箱：<a href="mailto:genggzy@gmail.com">genggzy@gmail.com</a></p>
            </section>
          </div>

          <!-- 底部动作条 -->
          <div class="policy-footer">
            <span class="footer-tip">生效日期：2026年9月18日</span>
            <button class="primary-btn" @click="$emit('close')">我已阅读并知悉</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.policy-mask {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(15, 23, 42, 0.6);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.policy-dialog {
  width: 100%;
  max-width: 620px;
  max-height: 82vh;
  background: var(--bg-card, #1e293b);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 16px;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.05);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: dialog-pop 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes dialog-pop {
  from {
    opacity: 0;
    transform: scale(0.96) translateY(8px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.policy-header {
  padding: 18px 20px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: rgba(255, 255, 255, 0.02);
}

.title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
}

.shield-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  background: rgba(16, 185, 129, 0.15);
  color: #10b981;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.dialog-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: #f8fafc;
}

.dialog-sub {
  margin: 2px 0 0;
  font-size: 11px;
  color: #94a3b8;
}

.close-btn {
  background: transparent;
  border: none;
  color: #94a3b8;
  cursor: pointer;
  padding: 6px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #f8fafc;
}

.policy-content {
  padding: 20px;
  overflow-y: auto;
  font-size: 13px;
  line-height: 1.65;
  color: #cbd5e1;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.policy-content::-webkit-scrollbar {
  width: 6px;
}

.policy-content::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
  border-radius: 3px;
}

.policy-content::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.25);
}

.notice-box {
  background: rgba(14, 165, 233, 0.1);
  border: 1px solid rgba(14, 165, 233, 0.25);
  border-radius: 10px;
  padding: 12px 14px;
  color: #e0f2fe;
}

.notice-tag {
  display: inline-block;
  background: #0284c7;
  color: #fff;
  font-size: 10px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 4px;
  margin-bottom: 6px;
  letter-spacing: 0.5px;
}

.notice-box p {
  margin: 0;
  font-size: 12px;
  line-height: 1.6;
}

.section h4 {
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 600;
  color: #38bdf8;
}

.section p {
  margin: 0 0 6px;
}

.section ul {
  margin: 0;
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.section li {
  line-height: 1.55;
}

.section code {
  background: rgba(255, 255, 255, 0.08);
  padding: 2px 5px;
  border-radius: 4px;
  font-size: 11px;
  color: #f1f5f9;
}

.contact-line a {
  color: #38bdf8;
  text-decoration: none;
}

.contact-line a:hover {
  text-decoration: underline;
}

.policy-footer {
  padding: 14px 20px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: rgba(255, 255, 255, 0.02);
}

.footer-tip {
  font-size: 11px;
  color: #64748b;
}

.primary-btn {
  background: linear-gradient(135deg, #0284c7 0%, #0369a1 100%);
  color: #ffffff;
  border: none;
  padding: 7px 18px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  box-shadow: 0 4px 12px rgba(2, 132, 199, 0.25);
  transition: all 0.15s;
}

.primary-btn:hover {
  opacity: 0.92;
  transform: translateY(-1px);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
