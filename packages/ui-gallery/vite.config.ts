import path from "node:path"
import {createRequire} from "node:module"

import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

import {themeBootstrap} from "../ui/vite/themeBootstrap.ts"

const require = createRequire(import.meta.url)
const nodePolyfillsRoot = path.dirname(path.dirname(require.resolve("vite-plugin-node-polyfills")))

export default defineConfig({
  plugins: [
    themeBootstrap({defaultTheme: "light", storageKey: "acton-ui-gallery-theme"}),
    react(),
    nodePolyfills({
      include: ["buffer"],
      globals: {
        Buffer: true,
      },
    }),
  ],
  resolve: {
    alias: {
      "@acton/transaction-ui": path.resolve(import.meta.dirname, "../transaction-ui/src"),
      "@acton/ui": path.resolve(import.meta.dirname, "../ui/src"),
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
    port: 3008,
  },
})
