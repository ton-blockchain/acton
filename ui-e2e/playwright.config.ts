import path from "node:path"
import process from "node:process"

import {defineConfig, devices} from "@playwright/test"

const repositoryRoot = path.resolve(import.meta.dirname, "..")
const localnetNodePort = Number(process.env.ACTON_UI_E2E_NODE_PORT ?? 15_411)
const localnetUiPort = Number(process.env.ACTON_UI_E2E_LOCALNET_UI_PORT ?? 14_306)
const explorerUiPort = Number(process.env.ACTON_UI_E2E_EXPLORER_UI_PORT ?? 14_307)

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
      name: "localnet-desktop",
      testMatch: /localnet\/.*\.spec\.ts/,
      use: {
        ...devices["Desktop Chrome"],
        baseURL: `http://127.0.0.1:${localnetUiPort}`,
        viewport: {width: 1440, height: 1000},
      },
    },
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
      command: `cargo run --bin acton -- localnet start --port ${localnetNodePort} --load-state ui-e2e/fixtures/localnet/ui-state.json --no-mining`,
      cwd: repositoryRoot,
      url: `http://127.0.0.1:${localnetNodePort}/acton_nodeInfo`,
      reuseExistingServer: false,
      timeout: 180_000,
    },
    {
      command: `VITE_LOCALNET_PROXY_TARGET=http://127.0.0.1:${localnetNodePort} bunx vite build && VITE_LOCALNET_PROXY_TARGET=http://127.0.0.1:${localnetNodePort} bunx vite preview --host 127.0.0.1 --port ${localnetUiPort}`,
      cwd: path.join(repositoryRoot, "crates/acton-localnet-ui"),
      port: localnetUiPort,
      reuseExistingServer: false,
      timeout: 60_000,
    },
    {
      command: `bunx vite build && bunx vite preview --host 127.0.0.1 --port ${explorerUiPort}`,
      cwd: path.join(repositoryRoot, "crates/acton-explorer-ui"),
      port: explorerUiPort,
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
})
