import {createRequire} from "node:module"
import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"

const require = createRequire(import.meta.url)

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  publicDir: "../../tasm-core/spec",
  optimizeDeps: {
    include: ["@codingame/monaco-vscode-monarch-service-override"],
  },
  resolve: {
    alias: [{find: /^vscode$/, replacement: require.resolve("vscode")}],
  },
  server: {
    port: 3021,
  },
})
