import path from "node:path"
import {createRequire} from "node:module"

import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

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
      "@acton/transaction-ui": path.resolve(import.meta.dirname, "../acton-transaction-ui/src"),
      "@tasm-spec": path.resolve(import.meta.dirname, "../tasm-core/spec"),
      "vite-plugin-node-polyfills/shims/buffer": path.resolve(
        nodePolyfillsRoot,
        "shims/buffer/index.ts",
      ),
    },
  },
  optimizeDeps: {
    exclude: ["tasm-web-wasm"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 3007,
  },
})
