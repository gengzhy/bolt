<script setup lang="ts">
// 模态弹窗：配对验证码比对 + 接收文件确认。
// 完成/失败状态不做弹窗，一律在右侧传输任务列表实时呈现。
import { useLt } from "../composables/useLt";
import { human } from "../utils/format";

const { pairReq, transReq, respondPair, respondTransfer } = useLt();
</script>

<template>
  <!-- 配对弹窗 -->
  <div v-if="pairReq" class="modal-mask">
    <!-- 发起配对端（发送端） -->
    <div v-if="pairReq.is_initiator" class="modal pair-modal initiator">
      <div class="pair-anim">
        <div class="pulse-ring"></div>
        <div class="spinner-ring"></div>
        <div class="spinner-center"></div>
      </div>
      <h3>设备配对中...</h3>
      <p>正在与 <b>{{ pairReq.name }}</b> 配对</p>
      <div class="code-container">
        <p class="code">{{ pairReq.code }}</p>
      </div>
      <p class="hint">请在对方设备屏幕上核对这 6 位验证码并确认接受</p>
      <div class="modal-actions">
        <button class="btn btn-cancel" @click="respondPair(pairReq.pair_id, false)">取消</button>
      </div>
    </div>

    <!-- 接收配对端（被连接端） -->
    <div v-else class="modal pair-modal receiver">
      <h3>配对请求</h3>
      <p><b>{{ pairReq.name }}</b> 请求配对</p>
      <div class="code-container">
        <p class="code">{{ pairReq.code }}</p>
      </div>
      <p class="hint">请在对方屏幕上核对这 6 位验证码是否一致</p>
      <div class="modal-actions">
        <button class="btn" @click="respondPair(pairReq.pair_id, false)">拒绝</button>
        <button class="btn primary" @click="respondPair(pairReq.pair_id, true)">验证码一致，接受</button>
      </div>
    </div>
  </div>

  <!-- 传输请求弹窗 -->
  <div v-else-if="transReq" class="modal-mask">
    <div class="modal">
      <h3>接收文件？</h3>
      <p><b>{{ transReq.name }}</b> 要发送 {{ transReq.file_count }} 个文件，共 {{ human(transReq.total_size) }}</p>
      <div class="modal-actions">
        <button class="btn" @click="respondTransfer(transReq.req_id, false)">拒绝</button>
        <button class="btn primary" @click="respondTransfer(transReq.req_id, true)">接受</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.28);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
  animation: fadeIn 0.15s ease-out;
}
@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.modal {
  background: var(--panel, #ffffff);
  border: 1px solid var(--line, rgba(0, 0, 0, 0.08));
  border-radius: var(--r-modal, 16px);
  padding: 24px 28px;
  min-width: 350px;
  max-width: 400px;
  text-align: center;
  box-shadow: 0 16px 36px rgba(0, 0, 0, 0.14), 0 4px 12px rgba(0, 0, 0, 0.05);
  animation: scaleUp 0.18s cubic-bezier(0.16, 1, 0.3, 1);
}
@keyframes scaleUp {
  from { transform: scale(0.94); opacity: 0.5; }
  to { transform: scale(1); opacity: 1; }
}

.modal h3 {
  margin: 0 0 8px;
  font-size: 17px;
  font-weight: 600;
  color: var(--text, #1e293b);
}
.modal p {
  font-size: 13px;
  color: var(--muted, #64748b);
  margin: 4px 0;
}
.modal p b {
  color: var(--text, #0f172a);
}

/* 现代雷达微光旋转动画 */
.pair-anim {
  width: 48px;
  height: 48px;
  margin: 0 auto 12px;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
}
.pulse-ring {
  position: absolute;
  inset: -6px;
  border-radius: 50%;
  border: 2px solid rgba(79, 142, 247, 0.28);
  animation: pulse 2.2s cubic-bezier(0.2, 0.8, 0.4, 1) infinite;
}
.spinner-ring {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  border: 3px solid rgba(79, 142, 247, 0.14);
  border-top-color: #4f8ef7;
  border-right-color: #4f8ef7;
  animation: spin 0.9s cubic-bezier(0.55, 0.15, 0.45, 0.85) infinite;
}
.spinner-center {
  position: absolute;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #4f8ef7;
  box-shadow: 0 0 10px rgba(79, 142, 247, 0.8);
}
@keyframes spin {
  to { transform: rotate(360deg); }
}
@keyframes pulse {
  0% { transform: scale(0.8); opacity: 0.9; }
  50% { transform: scale(1.22); opacity: 0.15; }
  100% { transform: scale(0.8); opacity: 0.9; }
}

/* 验证码卡片容器 */
.code-container {
  display: inline-flex;
  justify-content: center;
  background: rgba(79, 142, 247, 0.05);
  border: 1px dashed rgba(79, 142, 247, 0.35);
  border-radius: 12px;
  padding: 8px 24px;
  margin: 12px auto;
}
.code {
  font-size: 28px;
  letter-spacing: 8px;
  font-weight: 700;
  margin: 0 !important;
  font-family: Consolas, 'SF Mono', monospace;
  color: var(--primary, #4f8ef7);
}

.hint {
  color: var(--muted, #64748b);
  font-size: 12px;
  margin-top: 6px;
  line-height: 1.5;
}

.modal-actions {
  display: flex;
  justify-content: center;
  gap: 12px;
  margin-top: 20px;
}
.modal-actions .btn {
  min-width: 96px;
}

.btn-cancel {
  background: var(--panel, #ffffff);
  border: 1px solid var(--line, #cbd5e1);
  color: var(--muted, #64748b);
  border-radius: 8px;
  padding: 8px 24px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
}
.btn-cancel:hover {
  background: var(--bg-hover, #f1f5f9);
  color: var(--text, #1e293b);
  border-color: #94a3b8;
}
</style>
