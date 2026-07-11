import path from "node:path"
import {fileURLToPath} from "node:url"

import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"

const rootDir = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  plugins: [react()],
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
  },
  build: {
    outDir: path.join(rootDir, "dist/webview-ui"),
    emptyOutDir: false,
    target: "es2020",
    minify: "esbuild",
    lib: {
      entry: path.join(rootDir, "src/webview-ui/src/views/wallet/wallet-main.tsx"),
      name: "ActonWallet",
      formats: ["iife"],
      fileName: () => "wallet.js",
      cssFileName: "wallet",
    },
    rollupOptions: {
      output: {
        entryFileNames: "wallet.js",
        assetFileNames: "[name][extname]",
      },
    },
  },
})
