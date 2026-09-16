import { createApp } from "vue";
import App from "./App.vue";

// 全局禁止鼠标右键菜单
document.addEventListener("contextmenu", (e) => {
  e.preventDefault();
}, true);

createApp(App).mount("#app");
