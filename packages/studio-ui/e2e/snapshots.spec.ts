// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root
import {expect, test} from "@playwright/test"
import type {EnvironmentSnapshotOperation, StudioEnvironment} from "../src/studioApi"

const environment: StudioEnvironment = {
  id: "environment-1",
  name: "Snapshot test",
  status: "stopped",
  lifecycle: "managed",
  rpcUrl: "/rpc",
  config: {
    kind: "fullTonNetwork",
    apiV2Port: 18_002,
    apiV3Port: 8081,
    adminPort: 18_001,
    configPort: 18_000,
    observabilityPort: 18_007,
    nodes: [],
    importedAccounts: [],
    accounts: [],
  },
  capabilities: ["snapshots"],
  endpoints: {},
  network: {id: "local", label: "Local", chainId: -3, testOnly: true, supportsActions: false},
}

const snapshot = {
  id: "snapshot-test",
  name: "Baseline",
  createdAt: 1_788_752_600,
  sizeBytes: 42,
  stateSizeBytes: 80,
  masterchainSeqno: 42,
}

test.beforeEach(async ({page}) => {
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    const body = path.endsWith("/info")
      ? {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
      : path.endsWith("/environments")
        ? [environment]
        : []
    await route.fulfill({json: body})
  })
})

test("reload discovers progress even when inventory fails and recovers the list", async ({
  page,
}) => {
  let complete = false
  const startedAt = new Date().toISOString()
  await page.route("**/snapshot-operation", route =>
    route.fulfill({
      json: {
        kind: "create",
        phase: complete ? "completed" : "creatingArchive",
        startedAt,
        finishedAt: complete ? new Date().toISOString() : null,
        snapshotId: snapshot.id,
        snapshotName: snapshot.name,
        startupTimings: undefined,
        error: undefined,
      },
    }),
  )
  await page.route("**/api/v1/environments/environment-1/snapshots", route =>
    complete
      ? route.fulfill({json: [snapshot]})
      : route.fulfill({status: 409, json: {message: "Inventory is busy"}}),
  )

  await page.goto("/virtual-environments/environment-1/snapshots")
  await expect(
    page.locator("main").last().getByText("Creating snapshot", {exact: true}),
  ).toBeVisible()
  await expect(page.getByRole("button", {name: "Create snapshot", exact: true})).toBeDisabled()
  await expect(
    page
      .getByRole("region", {name: "Notifications"})
      .getByText("Failed to load snapshots", {exact: true}),
  ).toBeVisible()
  await expect(page.locator("main").last().getByText("Inventory is busy")).toHaveCount(0)

  await page.reload()
  await expect(
    page.locator("main").last().getByText("Creating snapshot", {exact: true}),
  ).toBeVisible()
  complete = true
  await expect(page.getByText(snapshot.name, {exact: true})).toBeVisible()
  await expect(page.getByRole("button", {name: "Create snapshot", exact: true})).toBeEnabled()
})

test("progress polling recovers after its initial request fails", async ({page}) => {
  let available = false
  await page.route("**/snapshot-operation", route =>
    available
      ? route.fulfill({
          json: {
            kind: "restore",
            phase: "restoringState",
            startedAt: new Date().toISOString(),
            finishedAt: undefined,
            snapshotId: snapshot.id,
            snapshotName: snapshot.name,
            startupTimings: undefined,
            error: undefined,
          },
        })
      : route.fulfill({status: 503, json: {message: "Progress unavailable"}}),
  )

  await page.goto("/virtual-environments/environment-1/snapshots")
  await expect(
    page
      .getByRole("region", {name: "Notifications"})
      .getByText("Failed to load snapshot progress", {exact: true}),
  ).toBeVisible()
  await expect(page.getByRole("button", {name: "Create snapshot", exact: true})).toBeDisabled()
  available = true
  await expect(
    page.locator("main").last().getByText("Restoring snapshot", {exact: true}),
  ).toBeVisible()
})

test("an older poll cannot replace a newly accepted snapshot operation", async ({page}) => {
  let operation: EnvironmentSnapshotOperation | null = null
  let posting = false
  let finishPost: (() => void) | undefined
  let finishPoll: (() => void) | undefined
  await page.route("**/snapshot-operation", async route => {
    const previous = operation
    if (posting)
      await new Promise<void>(resolve => {
        finishPoll = resolve
      })
    await route.fulfill({json: previous})
  })
  await page.route("**/api/v1/environments/environment-1/snapshots", async route => {
    if (route.request().method() !== "POST") return route.fulfill({json: []})
    posting = true
    await new Promise<void>(resolve => {
      finishPost = resolve
    })
    posting = false
    operation = {
      kind: "create",
      phase: "creatingArchive",
      startedAt: new Date().toISOString(),
      finishedAt: undefined,
      snapshotId: undefined,
      snapshotName: "New snapshot",
      startupTimings: undefined,
      error: undefined,
    }
    await route.fulfill({json: operation})
  })

  await page.goto("/virtual-environments/environment-1/snapshots")
  await page.getByRole("button", {name: "Create snapshot", exact: true}).click()
  const dialog = page.getByRole("dialog", {name: "Create snapshot", exact: true})
  await dialog.getByRole("textbox", {name: "Name"}).fill("New snapshot")
  await dialog.getByRole("button", {name: "Create snapshot", exact: true}).click()
  await expect.poll(() => Boolean(finishPoll)).toBe(true)
  finishPost?.()
  await expect(
    page.locator("main").last().getByText("Creating snapshot", {exact: true}),
  ).toBeVisible()
  finishPoll?.()
  // The closing dialog changes its title before its exit animation removes it.
  await expect(page.getByRole("dialog")).toHaveCount(0)
  await expect(page.getByRole("button", {name: "Create snapshot", exact: true})).toBeDisabled()
  await expect(
    page.locator("main").last().getByText("Creating snapshot", {exact: true}),
  ).toBeVisible()
})

for (const simulated of [false, true]) {
  test(`${simulated ? "Simulated" : "Full"} snapshot dialogs use sequential names and update the list before success`, async ({
    page,
  }) => {
    const selected: StudioEnvironment = simulated
      ? {
          ...environment,
          config: {
            kind: "actonSimulatedLocalnet",
            port: 5411,
            accounts: [],
            noMining: true,
            mineEmptyBlocks: false,
          },
        }
      : environment
    let saved = [{...snapshot, name: "Snapshot 1"}]
    let operation: EnvironmentSnapshotOperation | null = null
    let releaseList: (() => void) | undefined

    await page.route("**/api/v1/environments", route => route.fulfill({json: [selected]}))
    await page.route("**/snapshot-operation", route => route.fulfill({json: operation}))
    await page.route("**/api/v1/environments/environment-1/snapshots", async route => {
      if (route.request().method() === "POST") {
        const name = route.request().postDataJSON().name
        saved = [...saved, {...snapshot, id: "snapshot-new", name}]
        operation = {
          kind: "create",
          phase: "completed",
          startedAt: new Date().toISOString(),
          snapshotId: "snapshot-new",
          snapshotName: name,
        }

        return route.fulfill({json: operation})
      }

      if (saved.length > 1) {
        await new Promise<void>(resolve => {
          releaseList = resolve
        })
      }

      await route.fulfill({json: saved})
    })

    await page.goto("/virtual-environments/environment-1/snapshots")
    await page.getByRole("button", {name: "Create snapshot", exact: true}).click()
    const dialog = page.getByRole("dialog")
    await expect(dialog.getByRole("textbox", {name: "Name"})).toHaveValue("Snapshot 2")
    await expect(dialog.getByText(/^Save accounts/)).toBeVisible()
    await expect(dialog.getByRole("alert")).toHaveCount(simulated ? 0 : 1)

    await dialog.getByRole("button", {name: "Create snapshot", exact: true}).click()
    await expect.poll(() => Boolean(releaseList)).toBe(true)
    await expect(page.getByText("Snapshot created", {exact: true})).toHaveCount(0)
    releaseList?.()

    await expect(page.getByText("Snapshot created", {exact: true})).toBeVisible()
    await expect(page.getByRole("row").filter({hasText: "Snapshot 2"})).toBeVisible()
    if (simulated) await expect(page.getByText("Stop network", {exact: true})).toHaveCount(0)

    await page.getByRole("button", {name: "Restore snapshot", exact: true}).first().click()
    await expect(dialog.getByText("Restore the network state from this snapshot")).toBeVisible()
    await expect(dialog.getByRole("alert")).toHaveCount(simulated ? 0 : 1)
    await dialog.getByRole("button", {name: "Cancel", exact: true}).click()

    await page.getByRole("button", {name: "Delete Snapshot 1", exact: true}).click()
    await expect(dialog.getByRole("alert")).toHaveCount(0)
    await expect(dialog.getByText("This permanently deletes the saved snapshot")).toBeVisible()
  })
}
