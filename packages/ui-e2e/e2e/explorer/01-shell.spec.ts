import {expect, test} from "@playwright/test"

import {JETTON_MASTER_ADDRESS, mockJettonMaster} from "../support/jettonMaster"
import {
  expectVisualSnapshot,
  explorerLocalnetStorage,
  prepareVisualPage,
  visualSnapshotsEnabled,
} from "../support/visual"

test.describe("Explorer shell", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "explorer"})
    await page.goto("/")
    await expect(page.getByRole("link", {name: "actonscan"})).toBeVisible()
    await expect(page.getByRole("navigation", {name: "Explorer navigation"})).toBeVisible()
    await expect(page.getByRole("combobox", {name: "Explorer search"}).last()).toBeVisible()
  })

  test("renders the landing page and primary navigation", async ({page}) => {
    const primaryNavigation = page.getByRole("navigation", {name: "Explorer navigation"})
    await expect(primaryNavigation.getByRole("link", {name: "Blocks"})).toBeVisible()
    await expect(primaryNavigation.getByRole("link", {name: "ABI"})).toBeVisible()
    await expect(primaryNavigation.getByRole("link", {name: "Sources"})).toBeVisible()
    await expect(primaryNavigation.getByRole("link", {name: "Emulate"})).toHaveCount(0)
    await expect(page.getByRole("button", {name: "Mainnet"})).toBeVisible()

    const developerTools = page.getByRole("navigation", {name: "Developer tools"})
    await expect(developerTools.getByRole("link", {name: /Emulate/})).toHaveAttribute(
      "href",
      "/emulate",
    )
    await expect(developerTools.getByRole("link", {name: /Cell Inspector/})).toHaveAttribute(
      "href",
      "/cell",
    )
  })

  test("opens blocks by masterchain seqno and toncenter block ID", async ({page}) => {
    const search = page.getByRole("combobox", {name: "Explorer search"}).last()

    await search.fill("123")
    await search.press("Enter")
    await expect(page).toHaveURL(/\/block\/-1\/8000000000000000\/123$/)
    await expect(page.getByText("(-1,8000000000000000,123)", {exact: true})).toBeVisible()

    await page.goto("/")
    await search.fill("(0,A000000000000000,456)")
    await search.press("Enter")
    await expect(page).toHaveURL(/\/block\/0\/A000000000000000\/456$/)
    await expect(page.getByText("(0,A000000000000000,456)", {exact: true})).toBeVisible()
  })

  test("does not show the localnet-only mint action", async ({page}) => {
    await prepareVisualPage(page, {app: "explorer", storage: explorerLocalnetStorage()})
    await mockJettonMaster(page, true)
    await page.goto(`/address/${JETTON_MASTER_ADDRESS}`)

    await expect(page.getByRole("button", {name: "Metadata"})).toBeVisible()
    await expect(page.getByRole("button", {name: "Mint token"})).toHaveCount(0)
  })

  test.describe("visual snapshots", () => {
    test.skip(!visualSnapshotsEnabled, "Set CHECK_UI_SNAPSHOTS=1 on macOS")

    test("exp-shell-landing-light", async ({page}) => {
      await expectVisualSnapshot(page, "exp-shell-landing-light")
    })

    test("exp-shell-network-menu-open", async ({page}) => {
      await page.getByRole("button", {name: "Mainnet"}).click()
      await expect(page.getByRole("menu", {name: "Explorer network"})).toBeVisible()
      await expectVisualSnapshot(page, "exp-shell-network-menu-open")
    })

    test("exp-shell-network-add-open", async ({page}) => {
      await page.getByRole("button", {name: "Mainnet"}).click()
      await page.getByRole("button", {name: "Add network"}).click()
      await expect(page.getByLabel("Name")).toBeVisible()
      await expectVisualSnapshot(page, "exp-shell-network-add-open")
    })

    test("exp-shell-landing-dark", async ({page}) => {
      await prepareVisualPage(page, {app: "explorer", theme: "dark"})
      await page.reload()
      await expect(page.getByRole("button", {name: "Use light theme"})).toBeVisible()
      await expectVisualSnapshot(page, "exp-shell-landing-dark")
    })

    test("exp-shell-landing-mobile", async ({page}) => {
      await page.setViewportSize({width: 390, height: 844})
      await expectVisualSnapshot(page, "exp-shell-landing-mobile")
    })
  })
})
