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
  },
  preview: {
    port: 3015,
  },
})
