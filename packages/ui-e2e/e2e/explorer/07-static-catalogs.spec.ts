import {expect, test} from "@playwright/test"

import {expectVisualSnapshot, prepareVisualPage, visualSnapshotsEnabled} from "../support/visual"

test.describe("Explorer static catalogs", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "explorer"})
  })

  test("renders the ABI catalog", async ({page}) => {
    await page.goto("/abi")
    await expect(page.getByRole("heading", {name: "ABI", exact: true})).toBeVisible()
  })

  test("renders the sources catalog", async ({page}) => {
    await page.goto("/sources")
    await expect(page.getByRole("heading", {name: "Sources", exact: true})).toBeVisible()
  })

  test.describe("visual snapshots", () => {
    test.skip(!visualSnapshotsEnabled, "Set CHECK_UI_SNAPSHOTS=1 on macOS")

    test("exp-abi-catalog-default", async ({page}) => {
      await page.goto("/abi")
      await expect(page.getByRole("heading", {name: "ABI", exact: true})).toBeVisible()
      await expectVisualSnapshot(page, "exp-abi-catalog-default")
    })

    test("exp-sources-empty", async ({page}) => {
      await page.goto("/sources")
      await expect(page.getByRole("heading", {name: "Sources", exact: true})).toBeVisible()
      await expectVisualSnapshot(page, "exp-sources-empty")
    })
  })
})
