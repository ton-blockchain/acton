import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 3015,
    proxy: {
      "/api": "http://127.0.0.1:3016",
    },
  },
  preview: {
    port: 3015,
  },
})
