import react from "@vitejs/plugin-react"
import {defineConfig, loadEnv} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

import {themeBootstrap} from "../ui/vite/themeBootstrap.ts"

export default defineConfig(({mode}) => ({
  plugins: [
    themeBootstrap({storageKey: "ton-faucet-theme"}),
    react(),
    nodePolyfills({include: ["buffer"], globals: {Buffer: true}}),
  ],
  resolve: {
    dedupe: ["@acton/ui", "react", "react-dom"],
  },
  server: {
    port: 3008,
    proxy: {
      "^/(?:auth|challenge|claim|stats|openapi\\.json)(?:/|$)": {
        target:
          loadEnv(mode, import.meta.dirname).VITE_BACKEND_PROXY_TARGET || "https://faucet.ton.org",
        changeOrigin: true,
      },
    },
  },
}))
