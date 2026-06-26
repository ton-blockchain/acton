import path from "node:path"
import {createRequire} from "node:module"

import react from "@vitejs/plugin-react"
import {defineConfig, type PluginOption} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

const require = createRequire(import.meta.url)
const nodePolyfillsRoot = path.dirname(path.dirname(require.resolve("vite-plugin-node-polyfills")))

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
    port: 3000,
  },
})
