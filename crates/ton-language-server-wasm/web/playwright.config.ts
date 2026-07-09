import {defineConfig} from "@playwright/test"
import process from "node:process"

export default defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:3021",
  },
  webServer: {
    command: "bun run build:wasm && bun run dev",
    cwd: import.meta.dirname,
    reuseExistingServer: !process.env.CI,
    url: "http://127.0.0.1:3021",
  },
})
