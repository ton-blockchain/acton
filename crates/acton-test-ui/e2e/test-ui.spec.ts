import type {Page} from "@playwright/test"

import {
  expect,
  stabilizeVisualSnapshot,
  test,
  unionStorageTestName,
  type VisualSnapshotOptions,
} from "./support/acton-test-ui"

const visualSnapshotsEnabled =
  process.platform === "darwin" && Boolean(process.env.CHECK_UI_SNAPSHOTS)

interface StableScreenshotOptions extends VisualSnapshotOptions {
  readonly fitTestDetailsContent?: boolean
  readonly fullPage?: boolean
}

interface GasProfileReport {
  readonly total_gas: number
  readonly contracts: readonly unknown[]
  readonly tests?: readonly GasProfileTestReport[]
}

interface GasProfileTestReport {
  readonly name: string
  readonly total_gas: number
  readonly contracts: readonly unknown[]
}

const waitForNextFrame = async (page: Page) => {
  await page.evaluate(async () => {
    await new Promise<void>(resolve => {
      requestAnimationFrame(() => resolve())
    })
  })
}

const escapeRegExp = (value: string) => value.replaceAll(/[.*+?^${}()|[\]\\]/g, String.raw`\$&`)

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

const openOwnerCanSendJettons = async (page: Page) => {
  await page.getByRole("button", {name: /owner can send jettons/}).click()
  await expect(page.getByTestId("test-details-title")).toContainText("owner can send jettons")
}

const readGasProfile = async (page: Page, baseUrl: string): Promise<GasProfileReport> => {
  const response = await page.request.get(`${baseUrl}/api/gas-profile`)
  expect(response.ok()).toBeTruthy()
  return (await response.json()) as GasProfileReport
}

const expectGasProfileViewer = async (page: Page) => {
  const contractSelector = page.getByLabel("Contract gas profiles")
  const flamegraph = page.getByRole("img", {name: /Gas flamegraph for/})
  await expect(contractSelector).toBeVisible()
  await expect(contractSelector.getByRole("button").first()).toBeVisible()
  await expect(flamegraph).toBeVisible()
  await expect(flamegraph.locator("svg").first()).toBeVisible()
}

const openGlobalGasProfile = async (page: Page) => {
  const profileTab = page
    .getByRole("tablist", {name: "Main view"})
    .getByRole("tab", {name: "Profile"})
  await expect(profileTab).toBeVisible()
  await profileTab.click()
  await expect(profileTab).toHaveAttribute("aria-selected", "true")
  await expectGasProfileViewer(page)
}

const openSelectedTestGasProfile = async (page: Page) => {
  await openOwnerCanSendJettons(page)

  const profileTab = page
    .getByRole("tablist", {name: "Test details"})
    .getByRole("tab", {name: "Profile"})
  await expect(profileTab).toBeVisible()
  await profileTab.click()
  await expect(profileTab).toHaveAttribute("aria-selected", "true")
  await expectGasProfileViewer(page)
}

const openSelectedTestContractGasProfile = async (page: Page) => {
  await openSelectedTestGasProfile(page)

  const contractSelector = page.getByLabel("Contract gas profiles")
  await expect(contractSelector.getByRole("button", {name: "Tests"})).toBeVisible()

  const contractButton = contractSelector.getByRole("button", {name: "JettonWallet"})
  await expect(contractButton).toBeVisible()
  await contractButton.click()
  await expect(page.getByRole("img", {name: "Gas flamegraph for JettonWallet"})).toBeVisible()
}

const findGasProfileFrameIndex = async (page: Page): Promise<number> => {
  const flamegraph = page.getByTestId("gas-profile-flamegraph")
  return await flamegraph.evaluate(element => {
    const frames = [...element.querySelectorAll<SVGGElement>(".d3-flame-graph g.frame")].map(
      (frame, index) => {
        const datum = (
          frame as Readonly<{
            readonly __data__?: Readonly<{readonly data?: Record<string, unknown>}>
          }>
        ).__data__?.data
        const rect = frame.querySelector<SVGRectElement>("rect")

        return {
          index,
          name: typeof datum?.name === "string" ? datum.name : "",
          selfGas: typeof datum?.selfGas === "number" ? datum.selfGas : 0,
          width: rect?.getBoundingClientRect().width ?? 0,
        }
      },
    )
    if (frames.length === 0) {
      throw new Error("Expected gas profile flamegraph to render at least one frame")
    }

    const recvInternalFrame = frames.find(
      frame => frame.selfGas > 0 && frame.name === "recvInternal",
    )

    if (recvInternalFrame !== undefined) {
      return recvInternalFrame.index
    }

    const selfFrame = frames.find(frame => frame.index > 0 && frame.selfGas > 0 && frame.width > 24)

    return selfFrame?.index ?? frames.find(frame => frame.index > 0)?.index ?? 0
  })
}

const openSelectedTestContractGasProfileFrameHover = async (page: Page) => {
  await openSelectedTestContractGasProfile(page)

  const flamegraph = page.getByTestId("gas-profile-flamegraph")
  const frameIndex = await findGasProfileFrameIndex(page)
  const targetFrame = flamegraph.locator(".d3-flame-graph g.frame").nth(frameIndex)
  await expect(targetFrame).toBeVisible()
  await targetFrame.locator("rect").hover()
  await expect(page.locator(".d3-flame-graph-tip")).toBeVisible()
}

const openSelectedTestContractGasProfileInstructions = async (page: Page) => {
  await openSelectedTestContractGasProfile(page)

  const flamegraph = page.getByTestId("gas-profile-flamegraph")
  const frameIndex = await findGasProfileFrameIndex(page)
  const targetFrame = flamegraph.locator(".d3-flame-graph g.frame").nth(frameIndex)
  await expect(targetFrame).toBeVisible()
  await targetFrame.locator("rect").click()

  const frameDetails = page.getByTestId("gas-profile-frame-details")
  await expect(frameDetails).toBeVisible()
  await expect(frameDetails.getByText("Total", {exact: true})).toBeVisible()
  await frameDetails.getByRole("button", {name: "Show instruction statistics"}).click()

  const instructionTable = page.getByTestId("gas-profile-instruction-stats-table")
  await expect(instructionTable).toBeVisible()
  await expect(instructionTable.getByRole("columnheader", {name: "Instruction"})).toBeVisible()
  await expect(instructionTable.getByRole("columnheader", {name: "Gas"})).toBeVisible()
  await expect(instructionTable.getByRole("columnheader", {name: "Samples"})).toBeVisible()
}

const openSelectedTestContractGasProfileSelfInstructions = async (page: Page) => {
  await openSelectedTestContractGasProfileInstructions(page)

  const scopeControls = page.getByRole("group", {name: "Instruction statistics scope"})
  const selfButton = scopeControls.getByRole("button", {name: "Self"})
  await expect(selfButton).toBeVisible()
  await selfButton.click()
  await expect(selfButton).toHaveAttribute("aria-pressed", "true")
  await expect(
    page.getByTestId("gas-profile-instruction-stats-table").getByRole("columnheader", {
      name: "Share of Self",
    }),
  ).toBeVisible()
}

const openSelectedTestContractGasProfileStackOnlyInstructions = async (page: Page) => {
  await openSelectedTestContractGasProfileInstructions(page)

  const stackOnly = page.getByLabel("Stack only")
  await expect(stackOnly).toBeVisible()
  await stackOnly.check()
  await expect(stackOnly).toBeChecked()
  await expect(page.getByTestId("gas-profile-instruction-stats-table")).toBeVisible()
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

const openOwnerCanSendJettonsValueFlow = async (page: Page) => {
  await openOwnerCanSendJettons(page)

  await page.getByRole("tab", {name: "Transactions"}).click()
  await expect(page.getByRole("tab", {name: "Transactions"})).toHaveAttribute(
    "aria-selected",
    "true",
  )

  await page.getByRole("button", {name: "Trace 4"}).click()
  await expect(page.getByRole("button", {name: "Trace 4"})).toBeVisible()
  await page.getByRole("button", {name: "Show Value Flow"}).click()
  await expect(page.getByRole("button", {name: "Hide Value Flow"})).toBeVisible()

  const valueFlow = page.getByTestId("value-flow-section")
  await expect(valueFlow).toBeVisible()
  return valueFlow
}

const openOwnerCanSendJettonsTreasuryDeploys = async (page: Page) => {
  await openOwnerCanSendJettons(page)

  await page.getByRole("tab", {name: "Transactions"}).click()
  await expect(page.getByRole("tab", {name: "Transactions"})).toHaveAttribute(
    "aria-selected",
    "true",
  )

  const treasuryDeployToggle = page.getByRole("button", {name: /\d+ treasury deploys?/}).first()
  await expect(treasuryDeployToggle).toBeVisible()
  await treasuryDeployToggle.click()
  await expect(treasuryDeployToggle).toHaveAttribute("aria-expanded", "true")
  await expect(page.getByRole("button", {name: "Trace 1"})).toBeVisible()
  await expect(page.getByRole("button", {name: "Trace 4"})).toBeVisible()
}

const openOwnerCanSendJettonsFeeSummaryTreasuryDeploys = async (page: Page) => {
  await openOwnerCanSendJettons(page)
  await expect(page.getByRole("tab", {name: "Info"})).toHaveAttribute("aria-selected", "true")
  await expect(page.getByText("Fee Summary", {exact: true})).toBeVisible()

  const treasuryDeployToggle = page.getByRole("button", {name: /\d+ treasury deploys?/}).first()
  await expect(treasuryDeployToggle).toBeVisible()
  await treasuryDeployToggle.click()
  await expect(treasuryDeployToggle).toHaveAttribute("aria-expanded", "true")
  await expect(page.getByRole("button", {name: /Trace 1/})).toBeVisible()
  await expect(page.getByRole("button", {name: /Trace 4/})).toBeVisible()
}

const openTrace4FromFeeSummary = async (page: Page) => {
  await openOwnerCanSendJettons(page)
  await expect(page.getByRole("tab", {name: "Info"})).toHaveAttribute("aria-selected", "true")
  await expect(page.getByText("Fee Summary", {exact: true})).toBeVisible()

  await page.getByRole("button", {name: /Trace 4/}).click()
  await expect(page.getByRole("tab", {name: "Transactions"})).toHaveAttribute(
    "aria-selected",
    "true",
  )
  await expect(page.getByRole("button", {name: "Trace 4"})).toHaveAttribute("aria-current", "true")
  await expect(page.getByRole("button", {name: /^Transaction /}).first()).toBeVisible()
}

const collapseNavigation = async (page: Page) => {
  await page.getByRole("button", {name: /Collapse Sidebar/i}).click()
  await expect(page.getByTestId("sidebar-slot")).toHaveAttribute("aria-hidden", "true")
  await expect(page.getByRole("button", {name: "Expand sidebar"})).toBeVisible()
}

const openNavigationHoverPreview = async (page: Page) => {
  await collapseNavigation(page)
  await page.getByTestId("sidebar-peek-target").hover()
  await expect(page.getByTestId("sidebar-slot")).toHaveAttribute("aria-hidden", "false")
  await expect(page.getByPlaceholder("Filter tests...")).toBeVisible()
  await expect(page.getByRole("button", {name: /owner can send jettons/})).toBeVisible()
}

const openUnionStorageDiff = async (page: Page) => {
  await page.getByRole("button", {name: new RegExp(escapeRegExp(unionStorageTestName))}).click()
  await expect(page.getByTestId("test-details-title")).toContainText(unionStorageTestName)

  await page.getByRole("tab", {name: "Transactions"}).click()
  await expect(page.getByRole("tab", {name: "Transactions"})).toHaveAttribute(
    "aria-selected",
    "true",
  )

  const traceButtons = page.getByRole("button", {name: /^Trace /})
  const traceCount = await traceButtons.count()
  const traceIndexes = Array.from({length: Math.max(traceCount, 1)}).keys()

  for (const traceIndex of traceIndexes) {
    if (traceCount > 0) {
      await traceButtons.nth(traceIndex).click()
    }

    const transactions = page.getByRole("button", {name: /^Transaction /})
    await expect(transactions.first()).toBeVisible()
    const transactionCount = await transactions.count()

    for (const transactionIndex of Array.from({length: transactionCount}).keys()) {
      await transactions.nth(transactionIndex).click()

      const storageToggle = page.getByRole("button", {name: "Show storage state change"})
      if ((await storageToggle.count()) === 0) {
        continue
      }

      await storageToggle.first().click()
      const storageDiff = page.getByTestId("storage-diff-details")
      if ((await storageDiff.getByText("ActiveStorage", {exact: true}).count()) > 0) {
        await expect(storageDiff).toBeVisible()
        return storageDiff
      }
    }
  }

  throw new Error("Expected union storage switch transaction to expose an ActiveStorage diff")
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

  test("opens global gas profile for a profiled jetton run", async ({profiledActonUi, page}) => {
    await page.goto(profiledActonUi.baseUrl)

    const gasProfile = await readGasProfile(page, profiledActonUi.baseUrl)
    expect(gasProfile.total_gas).toBeGreaterThan(0)
    expect(gasProfile.contracts.length).toBeGreaterThan(0)

    await openGlobalGasProfile(page)
  })

  test("opens test gas profile tab when test execution profiling is enabled", async ({
    profiledActonUi,
    page,
  }) => {
    await page.goto(profiledActonUi.baseUrl)

    const gasProfile = await readGasProfile(page, profiledActonUi.baseUrl)
    const ownerTestProfile = gasProfile.tests?.find(profile =>
      profile.name.includes("owner can send jettons"),
    )
    expect(ownerTestProfile?.total_gas ?? 0).toBeGreaterThan(0)
    expect(ownerTestProfile?.contracts.length ?? 0).toBeGreaterThan(0)

    await openSelectedTestGasProfile(page)
  })

  test("switches selected test gas profile from test execution to contract", async ({
    profiledActonUi,
    page,
  }) => {
    await page.goto(profiledActonUi.baseUrl)

    await openSelectedTestContractGasProfile(page)
  })

  test("shows selected contract gas profile frame hover tooltip", async ({
    profiledActonUi,
    page,
  }) => {
    await page.goto(profiledActonUi.baseUrl)

    await openSelectedTestContractGasProfileFrameHover(page)
  })

  test("opens selected contract gas profile instruction table", async ({profiledActonUi, page}) => {
    await page.goto(profiledActonUi.baseUrl)

    await openSelectedTestContractGasProfileInstructions(page)
  })

  test("switches selected contract gas profile instructions to self scope", async ({
    profiledActonUi,
    page,
  }) => {
    await page.goto(profiledActonUi.baseUrl)

    await openSelectedTestContractGasProfileSelfInstructions(page)
  })

  test("switches selected contract gas profile instructions to stack only", async ({
    profiledActonUi,
    page,
  }) => {
    await page.goto(profiledActonUi.baseUrl)

    await openSelectedTestContractGasProfileStackOnlyInstructions(page)
  })

  test("opens value flow for a jetton transfer trace", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    const valueFlow = await openOwnerCanSendJettonsValueFlow(page)

    await expect(valueFlow.getByText("Account", {exact: true})).toBeVisible()
    await expect(valueFlow.getByText("Balance Change", {exact: true})).toBeVisible()
    await expect(valueFlow.getByText("Network Fee", {exact: true})).toBeVisible()
    await expect(valueFlow.getByText(/GRAM/).first()).toBeVisible()
  })

  test("opens treasury deploy traces for a jetton transfer trace", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    await openOwnerCanSendJettonsTreasuryDeploys(page)
  })

  test("opens treasury deploy fee rows for a jetton transfer trace", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    await openOwnerCanSendJettonsFeeSummaryTreasuryDeploys(page)
  })

  test("opens a trace from the fee summary table", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    await openTrace4FromFeeSummary(page)
  })

  test("collapses and expands navigation from a selected test", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    await openOwnerCanSendJettons(page)
    await collapseNavigation(page)

    await page.getByRole("button", {name: "Expand sidebar"}).click()
    await expect(page.getByTestId("sidebar-slot")).toHaveAttribute("aria-hidden", "false")
    await expect(page.getByRole("button", {name: /Collapse Sidebar/i})).toBeVisible()
  })

  test("opens navigation preview from the left edge hover target", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    await openOwnerCanSendJettons(page)
    await openNavigationHoverPreview(page)
  })

  test("shows storage diff for a union storage variant switch", async ({actonUi, page}) => {
    await page.goto(actonUi.baseUrl)

    const storageDiff = await openUnionStorageDiff(page)

    await expect(storageDiff.getByText("ActiveStorage", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("version:", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("owner:", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("balance:", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("quota:", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("limit:", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("enabled:", {exact: true})).toBeVisible()

    await expect(storageDiff.getByText("1", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("2", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("100", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("150", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("7", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("42", {exact: true})).toBeVisible()
    await expect(storageDiff.getByText("true", {exact: true})).toBeVisible()
  })

  test.describe("visual snapshots", () => {
    test.skip(
      !visualSnapshotsEnabled,
      "Set CHECK_UI_SNAPSHOTS to run visual snapshot checks on macOS",
    )

    test("matches primary states", async ({actonUi, page}) => {
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

      await page.getByRole("button", {name: "Trace 4"}).click()
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

    test("matches union storage diff", async ({actonUi, page}) => {
      await page.goto(actonUi.baseUrl)

      await openUnionStorageDiff(page)
      await expectStableScreenshot(page, "test-ui-union-storage-diff.png", {
        fitTestDetailsContent: true,
        fullPage: true,
      })
    })

    test("matches jetton value flow", async ({actonUi, page}) => {
      await page.goto(actonUi.baseUrl)

      await openOwnerCanSendJettonsValueFlow(page)
      await expectStableScreenshot(page, "test-ui-jetton-value-flow.png", {
        fitTestDetailsContent: true,
        fullPage: true,
      })
    })

    test("matches expanded treasury deploy traces", async ({actonUi, page}) => {
      await page.goto(actonUi.baseUrl)

      await openOwnerCanSendJettonsTreasuryDeploys(page)
      await expectStableScreenshot(page, "test-ui-expanded-treasury-deploys.png", {
        fitTestDetailsContent: true,
        fullPage: true,
      })
    })

    test("matches fee summary expanded treasury deploys", async ({actonUi, page}) => {
      await page.goto(actonUi.baseUrl)

      await openOwnerCanSendJettonsFeeSummaryTreasuryDeploys(page)
      await expectStableScreenshot(page, "test-ui-fee-summary-expanded-treasury-deploys.png", {
        fitTestDetailsContent: true,
        fullPage: true,
      })
    })

    test("matches navigation hover preview", async ({actonUi, page}) => {
      await page.goto(actonUi.baseUrl)

      await openOwnerCanSendJettons(page)
      await openNavigationHoverPreview(page)
      await expectStableScreenshot(page, "test-ui-navigation-hover-preview.png")
    })

    test("matches global gas profile", async ({profiledActonUi, page}) => {
      await page.goto(profiledActonUi.baseUrl)

      await openGlobalGasProfile(page)
      await expectStableScreenshot(page, "test-ui-global-gas-profile.png")
    })

    test("matches selected test gas profile", async ({profiledActonUi, page}) => {
      await page.goto(profiledActonUi.baseUrl)

      await openSelectedTestGasProfile(page)
      await expectStableScreenshot(page, "test-ui-selected-test-gas-profile.png", {
        fitTestDetailsContent: true,
        fullPage: true,
      })
    })

    test("matches selected test contract gas profile", async ({profiledActonUi, page}) => {
      await page.goto(profiledActonUi.baseUrl)

      await openSelectedTestContractGasProfile(page)
      await expectStableScreenshot(page, "test-ui-selected-test-contract-gas-profile.png", {
        fitTestDetailsContent: true,
        fullPage: true,
      })
    })

    test("matches selected test contract gas profile frame hover", async ({
      profiledActonUi,
      page,
    }) => {
      await page.goto(profiledActonUi.baseUrl)

      await openSelectedTestContractGasProfileFrameHover(page)
      await expectStableScreenshot(
        page,
        "test-ui-selected-test-contract-gas-profile-frame-hover.png",
        {
          fitTestDetailsContent: true,
          fullPage: true,
        },
      )
    })

    test("matches selected test contract gas profile instruction table", async ({
      profiledActonUi,
      page,
    }) => {
      await page.goto(profiledActonUi.baseUrl)

      await openSelectedTestContractGasProfileInstructions(page)
      await expectStableScreenshot(
        page,
        "test-ui-selected-test-contract-gas-profile-instructions.png",
        {
          fitTestDetailsContent: true,
          fullPage: true,
        },
      )
    })

    test("matches selected test contract gas profile self instructions", async ({
      profiledActonUi,
      page,
    }) => {
      await page.goto(profiledActonUi.baseUrl)

      await openSelectedTestContractGasProfileSelfInstructions(page)
      await expectStableScreenshot(
        page,
        "test-ui-selected-test-contract-gas-profile-self-instructions.png",
        {
          fitTestDetailsContent: true,
          fullPage: true,
        },
      )
    })

    test("matches selected test contract gas profile stack-only instructions", async ({
      profiledActonUi,
      page,
    }) => {
      await page.goto(profiledActonUi.baseUrl)

      await openSelectedTestContractGasProfileStackOnlyInstructions(page)
      await expectStableScreenshot(
        page,
        "test-ui-selected-test-contract-gas-profile-stack-only-instructions.png",
        {
          fitTestDetailsContent: true,
          fullPage: true,
        },
      )
    })
  })
})
