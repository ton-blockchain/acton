import path from "node:path"

import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

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
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 3007,
  },
})
