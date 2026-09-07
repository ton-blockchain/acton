import path from "node:path"
import {createRequire} from "node:module"

import react from "@vitejs/plugin-react"
import {defineConfig, loadEnv} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

import {gzipEmbeddedAssets} from "../ui/vite/embeddedAssets.ts"
import {themeBootstrap} from "../ui/vite/themeBootstrap.ts"

const require = createRequire(import.meta.url)
const nodePolyfillsRoot = path.dirname(path.dirname(require.resolve("vite-plugin-node-polyfills")))
const outputDirectory = path.resolve(import.meta.dirname, "dist")

export default defineConfig(({mode}) => ({
  plugins: [
    themeBootstrap({storageKey: "acton-studio-theme"}),
    react(),
    nodePolyfills({
      include: ["buffer", "path"],
      globals: {
        Buffer: true,
      },
    }),
    gzipEmbeddedAssets(outputDirectory),
  ],
  resolve: {
    alias: {
      "@acton/transaction-ui": path.resolve(import.meta.dirname, "../transaction-ui/src"),
      "vite-plugin-node-polyfills/shims/buffer": path.resolve(
        nodePolyfillsRoot,
        "shims/buffer/index.ts",
      ),
    },
  },
  build: {
    outDir: outputDirectory,
    emptyOutDir: true,
  },
  server: {
    port: 3015,
    proxy: {
      "/api": loadEnv(mode, import.meta.dirname).VITE_STUDIO_API_PROXY ?? "http://127.0.0.1:3016",
    },
  },
  preview: {
    port: 3015,
  },
}))
