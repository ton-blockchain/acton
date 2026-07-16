import {expect, test} from "@playwright/test"

import {expectVisualSnapshot, prepareVisualPage, visualSnapshotsEnabled} from "../support/visual"

test.describe("Explorer shell", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "explorer"})
    await page.goto("/")
    await expect(page.getByRole("link", {name: "actonscan"})).toBeVisible()
    await expect(page.getByRole("navigation", {name: "Explorer navigation"})).toBeVisible()
    await expect(page.getByPlaceholder("Search by address or hash").last()).toBeVisible()
  })

  test("renders the landing page and primary navigation", async ({page}) => {
    await expect(page.getByRole("link", {name: "Blocks"})).toBeVisible()
    await expect(page.getByRole("link", {name: "ABI"})).toBeVisible()
    await expect(page.getByRole("link", {name: "Sources"})).toBeVisible()
    await expect(page.getByRole("button", {name: "Mainnet"})).toBeVisible()
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
