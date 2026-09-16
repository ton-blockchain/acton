import react from "@vitejs/plugin-react"
import {defineConfig, loadEnv} from "vite"

import {themeBootstrap} from "../ui/vite/themeBootstrap.ts"

export default defineConfig(({mode}) => ({
  plugins: [themeBootstrap({storageKey: "ton-verifier-theme"}), react()],
  resolve: {
    dedupe: ["react", "react-dom"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 3007,
    proxy: {
      "^/api(?:/|$)": {
        target:
          loadEnv(mode, import.meta.dirname).VITE_BACKEND_PROXY_TARGET || "http://127.0.0.1:3000",
        changeOrigin: true,
      },
    },
  },
}))
