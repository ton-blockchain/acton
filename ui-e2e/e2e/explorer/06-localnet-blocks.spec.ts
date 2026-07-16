import {expect, test} from "@playwright/test"

import {
  expectVisualSnapshot,
  explorerLocalnetStorage,
  prepareVisualPage,
  visualSnapshotsEnabled,
} from "../support/visual"

test.describe("Explorer with the visual localnet network", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {
      app: "explorer",
      storage: explorerLocalnetStorage(),
    })
  })

  test("uses the selected custom network and renders real blocks", async ({page}) => {
    await page.goto("/blocks")
    await expect(page.getByRole("button", {name: "Visual localnet"})).toBeVisible()
    await expect(page.getByRole("region", {name: "Last masterchain blocks"})).toBeVisible()
  })

  test.describe("visual snapshots", () => {
    test.skip(!visualSnapshotsEnabled, "Set CHECK_UI_SNAPSHOTS=1 on macOS")

    test("exp-blocks-localnet-populated", async ({page}) => {
      await page.goto("/blocks")
      await expect(page.getByRole("region", {name: "Last masterchain blocks"})).toBeVisible()
      await expectVisualSnapshot(page, "exp-blocks-localnet-populated")
    })
  })
})
