// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"
import type {TestRunRecord} from "../src/studioApi"

const source = "get fun `test coverage`() {\n    return;\n}\n"
const filePath = "/project/tests/coverage.test.tolk"
const contract = {
  name: "Tests",
  total_gas: 123,
  sample_count: 1,
  samples: [
    {
      instruction_name: "RET",
      weight: 123,
      frames: [{function_name: "test coverage", url: filePath, line_number: 1, column_number: 0}],
    },
  ],
}
const profile = {
  total_gas: 123,
  contracts: [contract],
  tests: [{name: "test coverage", total_gas: 123, contracts: [contract]}],
}
const completed: TestRunRecord = {
  id: "with-artifacts",
  projectRoot: "/project",
  source: "manual",
  status: "passed",
  command: ["acton", "test", "--coverage", "--gas-profile", "gas.cpuprofile"],
  startedAt: "2026-09-15T10:00:00Z",
  finishedAt: "2026-09-15T10:00:01Z",
  exitCode: 0,
  stats: {total: 1, passed: 1, failed: 0, skipped: 0, todo: 0, durationMs: 1000},
  reports: [
    {
      name: "test coverage",
      suite_name: "coverage",
      file_path: filePath,
      row: 0,
      column: 0,
      status: "Passed",
      duration: {secs: 1, nanos: 0},
      gas_used: 123,
    },
  ],
}

for (const initiallyRunning of [false, true]) {
  test(`opens ${initiallyRunning ? "newly completed" : "saved"} coverage and gas profiles, then a run without artifacts`, async ({
    page,
  }) => {
    let currentRun: TestRunRecord = initiallyRunning
      ? {...completed, status: "running", finishedAt: undefined}
      : completed
    if (initiallyRunning) await page.clock.install()
    const without = {...completed, id: "without-artifacts"}
    const requests: string[] = []
    await page.route("**/api/v1/**", async route => {
      const url = new URL(route.request().url())
      const path = url.pathname
      requests.push(path)
      if (path.endsWith("/coverage.lcov")) {
        await route.fulfill({body: `TN:\nSF:${filePath}\nDA:2,1\nLF:1\nLH:1\nend_of_record\n`})
      } else if (path.endsWith("/gas-profile")) {
        await route.fulfill({json: profile})
      } else if (path.endsWith("/artifacts/file")) {
        expect(url.searchParams.get("path")).toBe(filePath)
        await route.fulfill({body: source})
      } else if (path.endsWith("/artifacts/config")) {
        const available = path.includes("/with-artifacts/") && currentRun.status === "passed"
        await route.fulfill({
          json: {
            project_root: "/project/",
            coverage_available: available,
            gas_profile_available: available,
          },
        })
      } else {
        await route.fulfill({
          json:
            path === "/api/v1/info"
              ? {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
              : path === "/api/v1/test-runs"
                ? [currentRun, without]
                : path === "/api/v1/test-runs/with-artifacts"
                  ? currentRun
                  : path === "/api/v1/test-runs/without-artifacts"
                    ? without
                    : path.endsWith("/output")
                      ? {stdout: "", stderr: ""}
                      : [],
        })
      }
    })

    await page.goto("/tests/with-artifacts")
    if (initiallyRunning) {
      await expect
        .poll(() => requests.filter(path => path.endsWith("/artifacts/config")).length)
        .toBe(1)
      await expect(page.getByRole("tab", {name: "Coverage", exact: true})).toHaveCount(0)
      await expect(page.getByRole("tab", {name: "Gas profile", exact: true})).toHaveCount(0)
      currentRun = completed
      await page.clock.fastForward(5001)
    }
    await page.getByRole("tab", {name: "Coverage", exact: true}).click()
    await expect(page.getByText("Line Coverage", {exact: true})).toBeVisible()
    await expect(
      page.getByLabel("Coverage source").getByText("1/1 executable lines", {exact: true}),
    ).toBeVisible()
    await expect(page.getByText("return;", {exact: true})).toBeVisible()

    await page.getByRole("tab", {name: "Gas profile", exact: true}).click()
    await expect(page.getByTestId("gas-profile-flamegraph")).toBeVisible()
    await page
      .getByRole("tablist", {name: "Test run views"})
      .getByRole("tab", {name: "Tests", exact: true})
      .click()
    await page.getByRole("tab", {name: "Profile", exact: true}).click()
    await expect(page.getByTestId("gas-profile-flamegraph")).toBeVisible()

    await page.getByRole("button", {name: "Local tests", exact: true}).click()
    await expect(page).toHaveURL(/\/tests$/)
    // Navigate through the client router so stale availability or profile state cannot survive.
    await page.getByRole("button", {name: "1 test", exact: true}).last().click()
    await expect(page.getByRole("tab", {name: "Info", exact: true})).toBeVisible()
    await expect(page.getByRole("tab", {name: "Coverage", exact: true})).toHaveCount(0)
    await expect(page.getByRole("tab", {name: "Profile", exact: true})).toHaveCount(0)
    await expect(page.getByRole("tab", {name: "Gas profile", exact: true})).toHaveCount(0)
    expect(requests.some(path => path.endsWith("/artifacts/coverage.lcov"))).toBe(true)
    expect(requests.some(path => path.endsWith("/artifacts/gas-profile"))).toBe(true)
    expect(requests.some(path => path.includes("/without-artifacts/artifacts/gas-profile"))).toBe(
      false,
    )
  })
}
