import path from "node:path"
import {createRequire} from "node:module"

import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

const localnetTarget = process.env.VITE_LOCALNET_PROXY_TARGET || "http://127.0.0.1:3010"
const require = createRequire(import.meta.url)
const nodePolyfillsRoot = path.dirname(path.dirname(require.resolve("vite-plugin-node-polyfills")))

export default defineConfig({
  plugins: [
    react(),
    nodePolyfills({
      include: ["buffer", "path"],
      globals: {
        Buffer: true,
      },
    }),
  ],
  resolve: {
    alias: {
      "@acton/shared-ui": path.resolve(import.meta.dirname, "../acton-shared-ui/src"),
      "@": path.resolve(import.meta.dirname, "../acton-shared-ui/src"),
      "@tasm-spec": path.resolve(import.meta.dirname, "../tasm-core/spec"),
      "ton-assembly": "@ton/tasm",
      "vite-plugin-node-polyfills/shims/buffer": path.resolve(
        nodePolyfillsRoot,
        "shims/buffer/index.ts",
      ),
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 3006,
    proxy: {
      "^/api(?:/|$)": {
        target: localnetTarget,
        changeOrigin: true,
      },
      "/acton_": {
        target: localnetTarget,
        changeOrigin: true,
      },
      "/emulate": {
        target: localnetTarget,
        changeOrigin: true,
      },
    },
  },
})
