import type {Page} from "@playwright/test"

import {
  expect,
  stabilizeVisualSnapshot,
  test,
  type VisualSnapshotOptions,
} from "./support/acton-test-ui"

interface StableScreenshotOptions extends VisualSnapshotOptions {
  readonly fitTestDetailsContent?: boolean
  readonly fullPage?: boolean
}

const waitForNextFrame = async (page: Page) => {
  await page.evaluate(async () => {
    await new Promise<void>(resolve => {
      requestAnimationFrame(() => resolve())
    })
  })
}

const fitViewportToTestDetailsContent = async (page: Page) => {
  const viewport = page.viewportSize()
  if (!viewport) {
    return
  }

  const targetHeight = await page.getByTestId("test-details-content").evaluate(element => {
    element.scrollTo({top: 0, left: 0})
    const rect = element.getBoundingClientRect()
    return Math.ceil(rect.top + element.scrollHeight + 16)
  })

  if (targetHeight > viewport.height) {
    await page.setViewportSize({width: viewport.width, height: targetHeight})
    await waitForNextFrame(page)
  }
}

const expectStableScreenshot = async (
  page: Page,
  name: string,
  options: StableScreenshotOptions = {},
) => {
  const {fitTestDetailsContent = false, fullPage = false, ...visualOptions} = options
  const originalViewport = page.viewportSize()

  if (fitTestDetailsContent) {
    await fitViewportToTestDetailsContent(page)
  }

  await stabilizeVisualSnapshot(page, visualOptions)
  try {
    await expect(page).toHaveScreenshot(name, {
      animations: "disabled",
      caret: "hide",
      fullPage,
      maxDiffPixels: 200,
    })
  } finally {
    if (fitTestDetailsContent && originalViewport) {
      await page.setViewportSize(originalViewport)
      await waitForNextFrame(page)
    }
  }
}

const openTrace4BodyAndActions = async (page: Page) => {
  await page.getByRole("button", {name: "Trace 4"}).click()
  await expect(page.getByRole("button", {name: "Trace 4"})).toBeVisible()

  const parsedBodyToggle = page.getByRole("button", {name: "Show parsed body"})
  const actionsToggle = page.getByRole("button", {name: "Show actions"})
  const transactions = page.getByRole("button", {name: /^Transaction /})
  const transactionCount = await transactions.count()

  const preferredTransactionIndices = [
    ...new Set([3, ...Array.from({length: transactionCount}).keys()]),
  ].filter(index => index < transactionCount)

  for (const index of preferredTransactionIndices) {
    await transactions.nth(index).click()

    if ((await parsedBodyToggle.count()) > 0 && (await actionsToggle.count()) > 0) {
      await parsedBodyToggle.first().click()
      await actionsToggle.first().click()

      const firstAction = page
        .getByRole("button", {name: /Send Message|Reserve|Set Code|Change Library/})
        .first()
      await expect(firstAction).toBeVisible()
      await firstAction.click()

      await expect(page.getByRole("button", {name: "Hide parsed body"})).toBeVisible()
      await expect(page.getByRole("button", {name: "Hide actions"})).toBeVisible()
      await expect(page.getByText("Actions Details", {exact: true})).toBeVisible()
      await expect(page.getByText("Details", {exact: true})).toBeVisible()
      return
    }
  }

  throw new Error("Expected Trace 4 to contain a transaction with parsed body and actions")
}

const openTrace4SendMessageAction = async (page: Page) => {
  const sendMessageAction = page.getByRole("button", {name: /Send Message/}).first()
  await expect(sendMessageAction).toBeVisible()
  await sendMessageAction.click()

  await expect(page.getByText("Message Data", {exact: true}).last()).toBeVisible()
  await expect(page.getByText("Mode:", {exact: true})).toBeVisible()
  await expect(page.getByText("To:", {exact: true})).toBeVisible()
}

test.describe("Test UI", () => {
  test("opens a real jetton test run and navigates test details", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    await expect(page.getByText("Test UI")).toBeVisible()
    await expect(page.getByTestId("summary-total")).toContainText("Total")
    await expect(page.getByTestId("summary-passed")).toContainText("Passed")
    await expect(page.getByRole("button", {name: /owner can send jettons/})).toBeVisible()

    await page.getByPlaceholder("Filter tests...").fill("owner can send")
    await expect(page.getByRole("button", {name: /owner can send jettons/})).toBeVisible()
    await expect(
      page.getByRole("button", {name: /deploy should create minter without bounce/}),
    ).toHaveCount(0)
    await page.getByPlaceholder("Filter tests...").fill("")

    await page.getByRole("button", {name: /owner can send jettons/}).click()
    await expect(page.getByTestId("test-details-title")).toContainText("wallet-behavior.test.tolk")
    await expect(page.getByTestId("test-details-title")).toContainText("owner can send jettons")
    await expect(page.getByRole("tab", {name: "Info"})).toHaveAttribute("aria-selected", "true")

    await page.getByRole("tab", {name: "Transactions"}).click()
    await expect(page.getByRole("tab", {name: "Transactions"})).toHaveAttribute(
      "aria-selected",
      "true",
    )

    const firstTransaction = page.getByRole("button", {name: /^Transaction /}).first()
    await expect(firstTransaction).toBeVisible()
    await firstTransaction.click()
    await expect(page.getByText("Message Route", {exact: true})).toBeVisible()
    await expect(page.getByText("Compute Phase", {exact: true})).toBeVisible()
  })

  test("opens coverage for the same jetton run", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    await page.getByRole("tab", {name: "Coverage"}).click()
    await expect(page.getByRole("tab", {name: "Coverage"})).toHaveAttribute("aria-selected", "true")
    await expect(page.getByText("Overall Score")).toBeVisible()
    await expect(page.getByText("Coverage Files")).toBeVisible()

    await page.getByPlaceholder("Filter files...").fill("JettonWallet")
    const walletFile = page.getByRole("button", {name: /JettonWallet\.tolk/}).first()
    await expect(walletFile).toBeVisible()
    await walletFile.click()

    await expect(
      page.getByRole("region", {name: "Coverage source"}).getByText(/JettonWallet\.tolk/),
    ).toBeVisible()
    await expect(page.getByText(/Score \d+\.\d%/)).toBeVisible()
  })

  test("matches visual snapshots for primary states", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    await page.getByRole("button", {name: /owner can send jettons/}).click()
    await expect(page.getByTestId("test-details-title")).toContainText("owner can send jettons")
    await expect(page.getByText("Fee Summary", {exact: true})).toBeVisible()
    await expectStableScreenshot(page, "test-ui-info.png")

    await page.getByPlaceholder("Filter tests...").fill("owner can send")
    await expect(page.getByRole("button", {name: /owner can send jettons/})).toBeVisible()
    await expectStableScreenshot(page, "test-ui-filtered-sidebar.png")
    await page.getByPlaceholder("Filter tests...").fill("")

    await page.getByRole("tab", {name: "Transactions"}).click()
    const firstTransaction = page.getByRole("button", {name: /^Transaction /}).first()
    await expect(firstTransaction).toBeVisible()
    await firstTransaction.click()
    await expect(page.getByText("Compute Phase", {exact: true})).toBeVisible()
    await expectStableScreenshot(page, "test-ui-transactions.png")

    await openTrace4BodyAndActions(page)
    await expectStableScreenshot(page, "test-ui-trace4-body-actions.png", {
      fitTestDetailsContent: true,
      fullPage: true,
    })
    await openTrace4SendMessageAction(page)
    await expectStableScreenshot(page, "test-ui-trace4-open-message.png", {
      fitTestDetailsContent: true,
      fullPage: true,
    })

    await page.getByRole("button", {name: "Trace 1"}).click()
    await page.getByRole("tab", {name: "Logs"}).click()
    await expect(page.getByText("VM Log", {exact: true}).first()).toBeVisible()
    await expectStableScreenshot(page, "test-ui-logs.png")

    await page.getByRole("tab", {name: "Coverage"}).click()
    await expect(page.getByText("Coverage Files", {exact: true})).toBeVisible()
    await page.getByPlaceholder("Filter files...").fill("JettonWallet")
    const walletFile = page.getByRole("button", {name: /JettonWallet\.tolk/}).first()
    await expect(walletFile).toBeVisible()
    await walletFile.click()
    await expect(page.getByRole("region", {name: "Coverage source"})).toContainText(
      "JettonWallet.tolk",
    )
    await expectStableScreenshot(page, "test-ui-coverage.png")

    await page.getByRole("button", {name: "Collapse sidebar"}).click()
    await expect(page.getByRole("button", {name: "Expand sidebar"})).toBeVisible()
    await expectStableScreenshot(page, "test-ui-collapsed-sidebar.png")

    await page.getByRole("button", {name: "Expand sidebar"}).click()
    await page.getByRole("button", {name: "Switch to dark theme"}).click()
    await expect(page.getByRole("button", {name: "Switch to light theme"})).toBeVisible()

    await page.getByRole("tab", {name: "Tests"}).click()
    await page.getByRole("tab", {name: "Info"}).click()
    await expect(page.getByText("Fee Summary", {exact: true})).toBeVisible()
    await expectStableScreenshot(page, "test-ui-dark-info.png", {theme: "dark"})

    await page.getByRole("tab", {name: "Transactions"}).click()
    await openTrace4BodyAndActions(page)
    await expectStableScreenshot(page, "test-ui-dark-trace4-body-actions.png", {
      fitTestDetailsContent: true,
      fullPage: true,
      theme: "dark",
    })
  })
})
