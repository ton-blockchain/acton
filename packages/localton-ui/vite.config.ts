import path from "node:path"

import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"

import {gzipEmbeddedAssets} from "../ui/vite/embeddedAssets.ts"
import {themeBootstrap} from "../ui/vite/themeBootstrap.ts"

const outputDirectory = path.resolve(import.meta.dirname, "dist")

export default defineConfig({
  plugins: [
    themeBootstrap({storageKey: "localton-observability-theme"}),
    react(),
    gzipEmbeddedAssets(outputDirectory),
  ],
  build: {
    outDir: outputDirectory,
    emptyOutDir: true,
  },
  server: {
    port: 3017,
    proxy: {
      "/api": "http://127.0.0.1:18007",
      "/healthz": "http://127.0.0.1:18007",
    },
  },
  preview: {
    port: 3017,
  },
})
