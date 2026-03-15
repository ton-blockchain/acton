import path from "node:path"

import react from "@vitejs/plugin-react"
import {defineConfig, type PluginOption} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

const apiProxyTarget = process.env.ACTON_TEST_UI_API_PROXY_TARGET
const apiProxy =
  apiProxyTarget === undefined
    ? undefined
    : {
        "/api": {
          target: apiProxyTarget,
          changeOrigin: false,
        },
      }

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
      "@acton/shared-ui": path.resolve(import.meta.dirname, "../acton-shared-ui/src"),
      "@": path.resolve(import.meta.dirname, "../acton-shared-ui/src"),
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 3000,
    proxy: apiProxy,
  },
  preview: {
    port: 4173,
    proxy: apiProxy,
  },
})
