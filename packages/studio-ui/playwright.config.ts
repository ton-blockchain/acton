// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {defineConfig} from "@playwright/test"

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  use: {baseURL: "http://127.0.0.1:14315", viewport: {width: 1360, height: 1000}},
  webServer: {
    command: "node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 14315",
    port: 14_315,
    reuseExistingServer: true,
  },
})
