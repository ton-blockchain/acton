import path from "node:path"
import process from "node:process"

import {defineConfig, devices} from "@playwright/test"

const repositoryRoot = path.resolve(import.meta.dirname, "../..")
const localnetNodePort = Number(process.env.ACTON_UI_E2E_NODE_PORT ?? 15_411)
const explorerUiPort = Number(process.env.ACTON_UI_E2E_EXPLORER_UI_PORT ?? 14_307)
const tonConnectDappPort = Number(process.env.ACTON_UI_E2E_TONCONNECT_DAPP_PORT ?? 14_308)
const tonConnectBridgePort = Number(process.env.ACTON_UI_E2E_TONCONNECT_BRIDGE_PORT ?? 14_309)
const tonConnectBridgeUrl = `http://127.0.0.1:${tonConnectBridgePort}/bridge`
const actonBinary = process.env.ACTON_E2E_BIN ?? path.join(repositoryRoot, "target/debug/acton")

export default defineConfig({
  testDir: "./e2e",
  outputDir: path.join(repositoryRoot, "test-results/acton-ui-e2e"),
  fullyParallel: false,
  timeout: 45_000,
  expect: {
    timeout: 5000,
    toHaveScreenshot: {
      pathTemplate:
        "{testDir}/__image_snapshots__/{projectName}/{testFileName}/{arg}{-snapshotSuffix}{ext}",
    },
  },
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: "list",
  use: {
    actionTimeout: 5000,
    colorScheme: "light",
    locale: "en-US",
    screenshot: "only-on-failure",
    timezoneId: "UTC",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "explorer-desktop",
      testMatch: /explorer\/.*\.spec\.ts/,
      use: {
        ...devices["Desktop Chrome"],
        baseURL: `http://127.0.0.1:${explorerUiPort}`,
        viewport: {width: 1440, height: 1000},
      },
    },
  ],
  webServer: [
    {
      command: `${JSON.stringify(actonBinary)} localnet start --port ${localnetNodePort} --load-state packages/ui-e2e/fixtures/localnet/ui-state.json --no-mining`,
      cwd: repositoryRoot,
      url: `http://127.0.0.1:${localnetNodePort}/acton_nodeInfo`,
      reuseExistingServer: false,
      timeout: 180_000,
    },
    {
      command: `bun run build && XDG_CONFIG_HOME=../../target/wrangler-config WRANGLER_LOG_PATH=../../target/wrangler-logs bunx wrangler pages dev dist --ip 127.0.0.1 --port ${explorerUiPort} --compatibility-date 2026-06-18`,
      cwd: path.join(repositoryRoot, "packages/explorer-ui"),
      port: explorerUiPort,
      reuseExistingServer: false,
      timeout: 60_000,
    },
    {
      command: `ACTON_UI_E2E_TONCONNECT_BRIDGE_PORT=${tonConnectBridgePort} bun run bridge-server.ts`,
      cwd: path.join(repositoryRoot, "packages/ui-e2e/fixtures/tonconnect-dapp"),
      url: `http://127.0.0.1:${tonConnectBridgePort}/health`,
      reuseExistingServer: false,
      timeout: 30_000,
    },
    {
      command: `VITE_TON_CONNECT_BRIDGE_URL=${tonConnectBridgeUrl} bunx vite --host 127.0.0.1 --port ${tonConnectDappPort}`,
      cwd: path.join(repositoryRoot, "packages/ui-e2e/fixtures/tonconnect-dapp"),
      port: tonConnectDappPort,
      reuseExistingServer: false,
      timeout: 30_000,
    },
  ],
})
