// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"
import {Address, beginCell} from "@ton/core"
import type {AdminOperation, AdminRequest, StudioEnvironment} from "../src/studioApi"

const environment: StudioEnvironment = {
  id: "environment-1",
  name: "Hardfork test",
  status: "running",
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
  },
  capabilities: ["explorer", "snapshots", "wallets"],
  endpoints: {},
  network: {id: "local", label: "Local", chainId: -3, testOnly: true, supportsActions: false},
}

test.beforeEach(async ({page}) => {
  await page.route("**/rpc/acton_listContracts", route => route.fulfill({json: []}))
})

test("admin form submits nanograms and tracks a detached operation across reload", async ({
  page,
}) => {
  let operation: AdminOperation | null = null
  let submitted: AdminRequest | undefined
  let status: StudioEnvironment["status"] = "running"
  let startingPolls = 0
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    let body: unknown = []
    if (path === "/api/v1/info")
      body = {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
    else if (path === "/api/v1/environments") {
      if (status === "starting") startingPolls += 1
      body = [{...environment, status}]
    } else if (path.endsWith("/admin")) {
      if (route.request().method() === "POST") {
        submitted = route.request().postDataJSON() as AdminRequest
        status = "starting"
        operation = {
          id: submitted.id,
          phase: "installing",
          startedAt: new Date().toISOString(),
          finishedAt: null,
          error: null,
          blockSeqno: null,
        }
      }
      body = operation
    }
    await route.fulfill({json: body})
  })
  await page.goto("/virtual-environments/environment-1/dashboard")
  const notifications = page.getByRole("region", {name: "Notifications"})
  const friendlyAddress = Address.parse(`0:${"11".repeat(32)}`).toString({testOnly: true})
  await page.getByLabel("State actions").getByRole("button", {name: "Admin actions"}).click()
  await expect(page).toHaveURL(/\/environment-1\/admin$/)
  await page.getByLabel("Account address").fill(`0:${"11".repeat(32)}`)
  await page.getByLabel("New balance").fill("12.5")
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  expect(submitted?.kind).toBe("accounts")
  if (submitted?.kind === "accounts")
    expect(submitted.edits[0]).toEqual({
      address: `0:${"11".repeat(32)}`,
      type: "balance",
      balance: "12500000000",
    })
  await expect(notifications.getByText("Installing hardfork", {exact: true})).toBeVisible()
  await expect(page.getByLabel("Account address")).toBeDisabled()
  await expect.poll(() => startingPolls).toBeGreaterThanOrEqual(2)
  await expect(page.getByLabel("Account address")).toHaveValue(friendlyAddress)
  await expect(page.getByLabel("New balance")).toHaveValue("12.5")
  await expect(page.getByLabel("Action", {exact: true})).toHaveValue("balance")
  await expect(page.locator("form")).not.toContainText("Installing hardfork")
  status = "running"
  operation = {
    id: submitted?.id ?? "",
    startedAt: new Date().toISOString(),
    error: null,
    phase: "completed",
    finishedAt: new Date().toISOString(),
    blockSeqno: 1234,
  }
  await expect(notifications.getByText("Changes applied", {exact: true})).toBeVisible()
  await expect(
    notifications.getByRole("link", {name: "#1234", includeHidden: true}),
  ).toHaveAttribute("href", "/virtual-environments/environment-1/block/-1/8000000000000000/1234")
  await expect(page.getByRole("button", {name: "Apply changes", exact: true})).toBeEnabled()
  await expect(page.getByLabel("Account address")).toHaveValue(friendlyAddress)
  await expect(page.getByLabel("New balance")).toHaveValue("12.5")
  await expect(page.getByLabel("Action", {exact: true})).toHaveValue("balance")

  operation = {...operation, phase: "indexing", finishedAt: null}
  await page.reload()
  await expect(notifications.getByText("Waiting for the indexer", {exact: true})).toBeVisible()
  await page.screenshot({path: "/tmp/acton-hardfork-review/admin-ui.png", fullPage: true})
})

test("ambiguous HTTP failure retries the exact same request", async ({page}) => {
  const requests: AdminRequest[] = []
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith("/admin") && route.request().method() === "POST") {
      requests.push(route.request().postDataJSON())
      if (requests.length === 1) {
        await route.abort()
        return
      }
      await route.fulfill({
        json: {
          id: requests[0].id,
          phase: "preparing",
          startedAt: new Date().toISOString(),
          finishedAt: null,
          error: null,
          blockSeqno: null,
        },
      })
      return
    }
    await route.fulfill({
      json: path.endsWith("/info")
        ? {protocolVersion: 1, serverVersion: "test"}
        : path.endsWith("/environments")
          ? [environment]
          : path.endsWith("/admin")
            ? null
            : [],
    })
  })
  await page.goto("/virtual-environments/environment-1/admin")
  await page.getByLabel("Account address").fill(`0:${"11".repeat(32)}`)
  await page.getByLabel("New balance").fill("7")
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  await page.getByRole("button", {name: "Retry same operation"}).click()
  expect(requests).toHaveLength(2)
  expect(requests[0]).toEqual(requests[1])
})

test("a stale poll cannot replace an acknowledged submission", async ({page}) => {
  let operation: AdminOperation | null = null
  let polls = 0
  let releasePoll: (() => void) | undefined
  const heldPoll = new Promise<void>(resolve => {
    releasePoll = resolve
  })

  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    let body: unknown = []

    if (path.endsWith("/info")) {
      body = {protocolVersion: 1, serverVersion: "test"}
    } else if (path.endsWith("/environments")) {
      body = [environment]
    } else if (path.endsWith("/admin")) {
      if (route.request().method() === "POST") {
        operation = {
          id: route.request().postDataJSON().id,
          phase: "installing",
          startedAt: new Date().toISOString(),
          finishedAt: null,
          error: null,
          blockSeqno: null,
        }
      } else {
        polls += 1

        if (polls === 2) {
          // This response observed the previous state before POST was accepted.
          await heldPoll
          await route.fulfill({json: null})
          return
        }
      }

      body = operation
    }

    await route.fulfill({json: body})
  })

  await page.goto("/virtual-environments/environment-1/admin")
  await page.getByLabel("Account address").fill(`0:${"11".repeat(32)}`)
  await page.getByLabel("New balance").fill("7")
  await expect.poll(() => polls).toBe(2)
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  await expect(page.getByText("Installing hardfork", {exact: true})).toBeVisible()

  // Record transient re-enabling too, even if the next poll repairs the state.
  await page.getByLabel("New balance").evaluate(input => {
    const observer = new MutationObserver(() => {
      if (!(input as HTMLInputElement).disabled) input.setAttribute("data-unlocked", "true")
    })
    observer.observe(input, {attributes: true, attributeFilter: ["disabled"]})
  })
  releasePoll?.()
  await expect.poll(() => polls).toBeGreaterThanOrEqual(3)

  await expect(page.getByLabel("New balance")).toBeDisabled()
  await expect(page.getByLabel("New balance")).not.toHaveAttribute("data-unlocked", "true")
})

for (const response of ["lost", "stale"] as const) {
  test(`polling completion wins over a ${response} POST response`, async ({page}) => {
    let operation: AdminOperation | null = null
    let releasePost: (() => void) | undefined
    const heldPost = new Promise<void>(resolve => {
      releasePost = resolve
    })

    await page.route("**/api/v1/**", async route => {
      const path = new URL(route.request().url()).pathname

      if (path.endsWith("/admin") && route.request().method() === "POST") {
        operation = {
          id: route.request().postDataJSON().id,
          phase: "completed",
          startedAt: new Date().toISOString(),
          finishedAt: new Date().toISOString(),
          error: null,
          blockSeqno: 1234,
        }

        await heldPost

        if (response === "lost") {
          await route.abort()
        } else {
          await route.fulfill({json: {...operation, phase: "preparing", finishedAt: null}})
        }

        return
      }

      await route.fulfill({
        json: path.endsWith("/info")
          ? {protocolVersion: 1, serverVersion: "test"}
          : path.endsWith("/environments")
            ? [environment]
            : path.endsWith("/admin")
              ? operation
              : [],
      })
    })

    await page.goto("/virtual-environments/environment-1/admin")
    await page.getByLabel("Account address").fill(`0:${"11".repeat(32)}`)
    await page.getByLabel("New balance").fill("7")
    const apply = page.getByRole("button", {name: "Apply changes", exact: true})
    await apply.click()
    await expect(page.getByText("Changes applied", {exact: true})).toBeVisible()

    releasePost?.()
    await expect(apply).toBeEnabled()
    await expect(page.getByText("Changes applied", {exact: true})).toBeVisible()
    await expect(page.getByText("Changes not submitted", {exact: true})).toHaveCount(0)
    await expect(page.getByText("Preparing operation", {exact: true})).toHaveCount(0)
  })
}

test("file loading blocks submission and switching actions discards its result", async ({page}) => {
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    await route.fulfill({
      json: path.endsWith("/info")
        ? {protocolVersion: 1, serverVersion: "test"}
        : path.endsWith("/environments")
          ? [environment]
          : path.endsWith("/admin")
            ? null
            : [],
    })
  })

  await page.goto("/virtual-environments/environment-1/admin")
  await page.getByLabel("Action", {exact: true}).selectOption("code")
  await page.getByLabel("Cell", {exact: true}).fill("previous value")
  await page.evaluate(() => {
    const arrayBuffer = File.prototype.arrayBuffer

    File.prototype.arrayBuffer = async function () {
      await new Promise<void>(resolve =>
        globalThis.addEventListener("release-boc", () => resolve(), {once: true}),
      )
      return arrayBuffer.call(this)
    }
  })
  await page.getByLabel("Load BoC file").setInputFiles({
    name: "cell.boc",
    mimeType: "application/octet-stream",
    buffer: beginCell().endCell().toBoc(),
  })

  const apply = page.getByRole("button", {name: "Apply changes", exact: true})
  await expect(page.getByText("Reading file…", {exact: true})).toBeVisible()
  await expect(apply).toBeDisabled()

  await page.getByLabel("Action", {exact: true}).selectOption("balance")
  await page.getByLabel("New balance").fill("42")
  await page.evaluate(() => globalThis.dispatchEvent(new Event("release-boc")))
  await expect(apply).toBeEnabled()
  await expect(page.getByLabel("New balance")).toHaveValue("42")
})

test("active admin operations pause suggestions and keep disabled fields consistent", async ({
  page,
}) => {
  let running = false
  let walletRequests = 0
  let contractRequests = 0
  const operation: AdminOperation = {
    id: "active-edit",
    phase: "preparing",
    startedAt: new Date().toISOString(),
    finishedAt: null,
    error: null,
    blockSeqno: null,
  }

  await page.route("**/rpc/acton_listContracts", async route => {
    contractRequests += 1
    await route.fulfill({status: running ? 200 : 503, json: []})
  })
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith("/wallets")) {
      walletRequests += 1
      await route.fulfill({status: running ? 200 : 409, json: []})
      return
    }
    await route.fulfill({
      json: path.endsWith("/info")
        ? {protocolVersion: 1, serverVersion: "test"}
        : path.endsWith("/environments")
          ? [{...environment, status: running ? "running" : "starting"}]
          : path.endsWith("/admin")
            ? operation
            : [],
    })
  })

  await page.goto("/virtual-environments/environment-1/admin")
  const notifications = page.getByRole("region", {name: "Notifications"})
  await expect(notifications.getByText("Preparing operation", {exact: true})).toBeVisible()
  await expect(page.getByRole("button", {name: "Apply changes", exact: true})).toHaveAttribute(
    "aria-busy",
    "true",
  )
  await page.evaluate(() => globalThis.dispatchEvent(new Event("focus")))
  await page.reload()
  await expect(notifications.getByText("Preparing operation", {exact: true})).toBeVisible()

  const backgrounds = await page
    .locator("#admin-action, input[aria-label='Account address'], #admin-value")
    .evaluateAll(elements => elements.map(element => getComputedStyle(element).backgroundColor))
  expect(new Set(backgrounds).size).toBe(1)
  expect({walletRequests, contractRequests}).toMatchInlineSnapshot(`
    {
      "contractRequests": 0,
      "walletRequests": 0,
    }
  `)
  await expect(notifications).not.toContainText("unavailable")
  await expect(notifications).not.toContainText("Failed to load wallets")

  running = true
  Object.assign(operation, {
    phase: "completed",
    finishedAt: new Date().toISOString(),
    blockSeqno: 12,
  })
  await expect(notifications.getByText("Changes applied", {exact: true})).toBeVisible()
  await expect.poll(() => walletRequests).toBe(1)
  await expect.poll(() => contractRequests).toBe(1)
})

test("historical administrative errors are not shown again on page open or reload", async ({
  page,
}) => {
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    await route.fulfill({
      json: path.endsWith("/info")
        ? {protocolVersion: 1, serverVersion: "test"}
        : path.endsWith("/environments")
          ? [{...environment, status: "failed", error: "Recovery needs attention"}]
          : path.endsWith("/admin")
            ? {
                id: "failed-edit",
                phase: "failed",
                startedAt: new Date().toISOString(),
                finishedAt: new Date().toISOString(),
                error: "Cold backup could not be restored",
                blockSeqno: null,
              }
            : [],
    })
  })
  await page.goto("/virtual-environments/environment-1/admin")
  await expect(page.getByRole("button", {name: "Apply changes", exact: true})).toBeDisabled()
  await expect(page.getByText("Cold backup could not be restored", {exact: true})).toHaveCount(0)
  await expect(page.locator("form")).not.toContainText("Start the environment")
  await expect(page.getByText("Operation failed", {exact: true})).toHaveCount(0)
  await page.reload()
  await expect(page.getByLabel("Account address")).toBeVisible()
  await expect(page.getByText("Cold backup could not be restored", {exact: true})).toHaveCount(0)
})

test("validation and operation failures use toasts without repeating after dismissal", async ({
  page,
}) => {
  let operation: AdminOperation | null = null
  let submissions = 0
  let submittedId = ""
  let polls = 0
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith("/admin")) {
      if (route.request().method() === "POST") {
        submissions += 1
        submittedId = route.request().postDataJSON().id
        operation = {
          id: submittedId,
          phase: "preparing",
          startedAt: new Date().toISOString(),
          finishedAt: null,
          error: null,
          blockSeqno: null,
        }
      } else polls += 1
      await route.fulfill({json: operation})
      return
    }
    await route.fulfill({
      json: path.endsWith("/info")
        ? {protocolVersion: 1, serverVersion: "test"}
        : path.endsWith("/environments")
          ? [environment]
          : [],
    })
  })

  await page.goto("/virtual-environments/environment-1/admin")
  const notifications = page.getByRole("region", {name: "Notifications"})
  await page.getByLabel("Account address").fill("123")
  await page.getByLabel("New balance").fill("123")
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  await expect(
    notifications.getByText("Enter a valid raw or friendly account address"),
  ).toBeVisible()
  expect(submissions).toBe(0)
  await expect(page.locator("form [role=alert]")).toHaveCount(0)
  await notifications
    .getByRole("button", {name: "Dismiss notification", includeHidden: true})
    .click()

  await page.getByLabel("Account address").fill(`0:${"11".repeat(32)}`)
  await page.getByLabel("New balance").fill("-1")
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  await expect(
    notifications.getByText("Enter a nonnegative GRAM amount with at most 9 decimal places"),
  ).toBeVisible()
  expect(submissions).toBe(0)
  await notifications
    .getByRole("button", {name: "Dismiss notification", includeHidden: true})
    .click()

  await page.getByLabel("New balance").fill("1")
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  await expect(notifications.getByText("Preparing operation", {exact: true})).toBeVisible()
  operation = {
    id: submittedId,
    startedAt: new Date().toISOString(),
    blockSeqno: null,
    phase: "failed",
    finishedAt: new Date().toISOString(),
    error: "Indexer did not catch up",
  }
  await expect(notifications.getByText("Indexer did not catch up", {exact: true})).toBeVisible()
  await notifications
    .getByRole("button", {name: "Dismiss notification", includeHidden: true})
    .click()
  const previousPolls = polls
  await expect.poll(() => polls, {timeout: 10_000}).toBeGreaterThan(previousPolls + 2)
  await expect(page.getByText("Indexer did not catch up", {exact: true})).toHaveCount(0)
  await page.reload()
  await expect(page.getByRole("button", {name: "Apply changes", exact: true})).toBeEnabled()
  await expect(page.getByText("Indexer did not catch up", {exact: true})).toHaveCount(0)
})

test("admin cell edits accept common BoC encodings and binary files", async ({page}) => {
  const requests: AdminRequest[] = []
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith("/admin")) {
      if (route.request().method() === "POST") {
        const request = route.request().postDataJSON()
        requests.push(request)
        await route.fulfill({
          json: {
            id: request.id,
            phase: "completed",
            startedAt: new Date().toISOString(),
            finishedAt: new Date().toISOString(),
            error: null,
            blockSeqno: 1234,
          },
        })
      } else await route.fulfill({json: null})
      return
    }
    await route.fulfill({
      json: path.endsWith("/info")
        ? {protocolVersion: 1, serverVersion: "test"}
        : path.endsWith("/environments")
          ? [environment]
          : [],
    })
  })

  await page.goto("/virtual-environments/environment-1/admin")
  await page.getByLabel("Account address").fill(`0:${"11".repeat(32)}`)
  await page.getByLabel("Action", {exact: true}).selectOption("data")
  const cell = beginCell().storeUint(65_535, 16).storeRef(beginCell().storeUint(42, 8)).endCell()
  const bytes = cell.toBoc()
  const base64 = bytes.toString("base64")
  const hex = bytes.toString("hex")
  const variants = [
    base64,
    bytes.toString("base64url"),
    hex,
    `0x${hex.replace(/(.{8})/g, "$1\n")}`,
    cell.toBoc({idx: true, crc32: false}).toString("base64"),
    `ton://cell/${base64}`,
    `https://example.com/inspect?boc=${encodeURIComponent(base64)}`,
  ]

  for (const value of variants) {
    await page.getByLabel("Cell", {exact: true}).fill(value)
    await page.getByRole("button", {name: "Apply changes", exact: true}).click()
    await expect(page.getByRole("button", {name: "Apply changes", exact: true})).toBeEnabled()
    expect(requests.at(-1)?.edits[0]).toEqual({
      address: `0:${"11".repeat(32)}`,
      type: "data",
      boc: base64,
    })
    await expect(page.getByLabel("Cell", {exact: true})).toHaveValue(value)
    await page
      .getByRole("region", {name: "Notifications"})
      .getByRole("button", {name: "Dismiss notification", includeHidden: true})
      .click()
    await expect(
      page.getByRole("region", {name: "Notifications"}).getByText("Changes applied", {exact: true}),
    ).toHaveCount(0)
  }
  expect(requests).toHaveLength(variants.length)

  await page
    .getByLabel("Load BoC file")
    .setInputFiles({name: "state.boc", mimeType: "application/octet-stream", buffer: bytes})
  await expect(page.getByLabel("Cell", {exact: true})).toHaveValue(base64)
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  await expect.poll(() => requests.length).toBe(variants.length + 1)
  expect(requests.at(-1)?.edits[0]).toEqual({
    address: `0:${"11".repeat(32)}`,
    type: "data",
    boc: base64,
  })

  await page.getByLabel("Cell", {exact: true}).fill("not a BoC")
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  await expect(
    page.getByRole("region", {name: "Notifications"}).getByText("Changes not submitted"),
  ).toBeVisible()
  expect(requests).toHaveLength(variants.length + 1)
})
