import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Settings recommended for Tauri: fixed port, no screen clearing, and no
// watching of the Rust sources.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**", "**/res/**"] },
  },
  // The lazily loaded 3D viewer chunk is mostly three.js.
  build: { target: "es2022", chunkSizeWarningLimit: 700 },
});
