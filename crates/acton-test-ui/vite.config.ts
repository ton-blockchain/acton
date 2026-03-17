import path from "node:path"

import react from "@vitejs/plugin-react"
import {defineConfig, type PluginOption} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

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
      "@ton/core": path.resolve(import.meta.dirname, "../../node_modules/@ton/core"),
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
