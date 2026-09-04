import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri v2 约定：dev 端口固定；dist 输出到 tauri_app/dist，
// 与 src-tauri/tauri.conf.json 的 frontendDist("../dist") 对应。
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "chrome105",
    outDir: "dist",
    emptyOutDir: true,
  },
});
