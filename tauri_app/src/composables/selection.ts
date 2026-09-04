import { ref } from "vue";

// 发送目标设备（左栏卡片点选 → 中栏发送区使用），模块级共享状态。
export const targetUuid = ref("");
