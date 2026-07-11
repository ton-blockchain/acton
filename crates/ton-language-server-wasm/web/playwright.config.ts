import {defineConfig} from "@playwright/test"

export default defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:3023",
  },
  webServer: {
    command: "bun run build:wasm && bunx vite --host 127.0.0.1 --port 3023 --force",
    cwd: import.meta.dirname,
    reuseExistingServer: false,
    url: "http://127.0.0.1:3023",
  },
})
