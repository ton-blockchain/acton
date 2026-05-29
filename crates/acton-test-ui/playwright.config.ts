import path from "node:path"

import {defineConfig} from "@playwright/test"

const repositoryRoot = path.resolve(import.meta.dirname, "../..")

export default defineConfig({
  testDir: "./e2e",
  outputDir: path.join(repositoryRoot, "test-results/acton-test-ui-e2e"),
  fullyParallel: false,
  workers: 1,
  timeout: 90_000,
  expect: {
    timeout: 10_000,
  },
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: "list",
  use: {
    actionTimeout: 10_000,
    browserName: "chromium",
    colorScheme: "light",
    locale: "en-US",
    screenshot: "only-on-failure",
    timezoneId: "UTC",
    trace: "retain-on-failure",
    viewport: {width: 1280, height: 900},
  },
})
