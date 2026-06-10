import type {Page} from "@playwright/test"

import {
  expect,
  fanoutGraphVisualScenarios,
  stabilizeVisualSnapshot,
  test,
} from "./support/acton-test-ui"

const visualSnapshotsEnabled =
  process.platform === "darwin" && Boolean(process.env.CHECK_UI_SNAPSHOTS)

const waitForNextFrame = async (page: Page) => {
  await page.evaluate(async () => {
    await new Promise<void>(resolve => {
      requestAnimationFrame(() => resolve())
    })
  })
}

const escapeRegExp = (value: string) => value.replaceAll(/[.*+?^${}()|[\]\\]/g, String.raw`\$&`)

const measureGraphContent = async (page: Page): Promise<{height: number; width: number}> => {
  return await page.getByTestId("test-details-content").evaluate(element => {
    element.scrollTo({top: 0, left: 0})

    const contentRect = element.getBoundingClientRect()
    const graph = element.querySelector<HTMLElement>(".rd3t-tree-container")
    const treeContainer = graph?.closest("[class*='treeContainer']") as HTMLElement | null
    const treeRect = treeContainer?.getBoundingClientRect()
    const horizontalContent =
      (treeContainer?.scrollWidth ?? graph?.scrollWidth ?? 0) +
      (treeRect?.left ?? contentRect.left) +
      16

    return {
      height: Math.ceil(contentRect.top + element.scrollHeight + 16),
      width: Math.ceil(
        Math.max(
          document.documentElement.scrollWidth,
          document.body.scrollWidth,
          horizontalContent,
        ),
      ),
    }
  })
}

const fitViewportToGraphContent = async (page: Page) => {
  const originalViewport = page.viewportSize()
  if (!originalViewport) {
    return
  }

  let currentViewport = originalViewport
  for (let i = 0; i < 2; i += 1) {
    const target = await measureGraphContent(page)
    const nextViewport = {
      width: Math.max(currentViewport.width, target.width),
      height: Math.max(currentViewport.height, target.height),
    }

    if (
      nextViewport.width === currentViewport.width &&
      nextViewport.height === currentViewport.height
    ) {
      return
    }

    await page.setViewportSize(nextViewport)
    await waitForNextFrame(page)
    currentViewport = nextViewport
  }
}

const stabilizeFanoutTransactionDetails = async (page: Page) => {
  await page.evaluate(() => {
    const feeValueTitles = new Set(["End Balance", "Total Fee", "Action Fee", "Forward Fee"])
    const rows = [
      ...document.querySelectorAll<HTMLElement>(
        "[class*='labeledSectionRow'], [class*='detailRow']",
      ),
    ]
    const feesRow = rows.find(row => {
      const title = row.querySelector<HTMLElement>(
        "[class*='labeledSectionTitle'], [class*='detailLabel']",
      )
      return title?.textContent?.trim() === "Fees & Sent"
    })

    if (!feesRow) {
      return
    }

    for (const item of feesRow.querySelectorAll<HTMLElement>("[class*='multiColumnItem']")) {
      const title = item.querySelector<HTMLElement>("[class*='multiColumnItemTitle']")
      if (!feeValueTitles.has(title?.textContent?.trim() ?? "")) {
        continue
      }

      item
        .querySelector<HTMLElement>("[class*='multiColumnItemValue']")
        ?.replaceChildren(document.createTextNode("<amount>"))
    }
  })
}

const expectStableGraphScreenshot = async (page: Page, name: string) => {
  const originalViewport = page.viewportSize()

  try {
    await fitViewportToGraphContent(page)
    await stabilizeVisualSnapshot(page)
    await stabilizeFanoutTransactionDetails(page)
    await expect(page).toHaveScreenshot(name, {
      animations: "disabled",
      caret: "hide",
      fullPage: true,
      maxDiffPixels: 200,
    })
  } finally {
    if (originalViewport) {
      await page.setViewportSize(originalViewport)
      await waitForNextFrame(page)
    }
  }
}

const openFanoutGraphScenario = async (
  page: Page,
  scenario: (typeof fanoutGraphVisualScenarios)[number],
) => {
  await page.getByRole("button", {name: new RegExp(escapeRegExp(scenario.testName))}).click()
  await expect(page.getByTestId("test-details-title")).toContainText(scenario.testName)

  const transactionsTab = page.getByRole("tab", {name: "Transactions"})
  await transactionsTab.click()
  await expect(transactionsTab).toHaveAttribute("aria-selected", "true")

  const traceTab = page.getByRole("button", {name: scenario.traceName})
  await expect(traceTab).toBeVisible()
  await traceTab.click()
  await expect(traceTab).toHaveAttribute("aria-current", "true")

  await expect(page.locator(".rd3t-tree-container svg").first()).toBeVisible()
  const firstTransaction = page.getByRole("button", {name: /^Transaction /}).first()
  await expect(firstTransaction).toBeVisible()
  await firstTransaction.click()
  await expect(page.getByText("Message Route", {exact: true})).toBeVisible()
}

test.describe("Fanout graph visual snapshots", () => {
  test.skip(
    !visualSnapshotsEnabled,
    "Set CHECK_UI_SNAPSHOTS to run fanout graph visual snapshot checks on macOS",
  )

  for (const scenario of fanoutGraphVisualScenarios) {
    test(`matches ${scenario.traceName}`, async ({fanoutGraphUi, page}) => {
      await page.goto(fanoutGraphUi.baseUrl)

      await openFanoutGraphScenario(page, scenario)
      await expectStableGraphScreenshot(page, scenario.snapshotName)
    })
  }
})
