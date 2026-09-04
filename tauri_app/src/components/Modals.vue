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
    <div class="modal">
      <h3>配对请求</h3>
      <p><b>{{ pairReq.name }}</b> 请求配对</p>
      <p class="code">{{ pairReq.code }}</p>
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
  background: rgba(0, 0, 0, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
}
.modal {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--r-modal);
  padding: 24px 28px;
  min-width: 340px;
  text-align: center;
  box-shadow: var(--shadow-hover), var(--shadow-1);
}
.modal h3 { margin: 0 0 8px; font-size: 16px; }
.modal p { font-size: 13px; color: var(--muted); }
.modal p b { color: var(--text); }
.code {
  font-size: 30px;
  letter-spacing: 6px;
  font-weight: 700;
  margin: 14px 0;
  font-family: Consolas, monospace;
  color: var(--text);
}
.hint { color: var(--muted); font-size: 12px; }
.modal-actions { display: flex; justify-content: center; gap: 12px; margin-top: 18px; }
.modal-actions .btn { min-width: 96px; }
</style>
