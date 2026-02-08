import react from "@vitejs/plugin-react"
import { defineConfig, type PluginOption } from "vite"
import { nodePolyfills } from "vite-plugin-node-polyfills"
import path from "path"

export default defineConfig({
  plugins: [
    react() as PluginOption,
    nodePolyfills({
      include: ["buffer", "path"],
      globals: {
        Buffer: true,
      },
    }) as PluginOption,
  ],
  resolve: {
    alias: {
      "@acton/shared-ui": path.resolve(__dirname, "../acton-shared-ui/src"),
      "@": path.resolve(__dirname, "../acton-shared-ui/src"),
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 3000,
  },
})
