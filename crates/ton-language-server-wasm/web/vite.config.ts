import {createRequire} from "node:module"
import {defineConfig} from "vite"

const require = createRequire(import.meta.url)

export default defineConfig({
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  resolve: {
    alias: [{find: /^vscode$/, replacement: require.resolve("vscode")}],
  },
  server: {
    port: 3021,
  },
})
