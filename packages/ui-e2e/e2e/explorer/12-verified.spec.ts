import {expect, test} from "@playwright/test"

import {prepareVisualPage} from "../support/visual"

const VERIFIED_ITEMS = Array.from({length: 50}, (_, index) => {
  const sequence = index + 1
  return {
    code_hash: sequence.toString(16).padStart(64, "0"),
    source_bundle_hash: (sequence + 100).toString(16).padStart(64, "0"),
    verified_at: 1_750_000_000 - sequence,
    storage_revision: String(sequence),
    entrypoint: `contracts/contract-${sequence}.tolk`,
    compiler: {
      language: "tolk",
      version: "1.4.1",
      params: {},
    },
    file_count: 1,
    has_tolk_abi: false,
    abi_name: `Contract ${sequence}`,
  }
})

test.describe("Verified contracts", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "explorer"})
    await page.route("https://verifier.acton.monster/api/v1/**", async route => {
      const url = new URL(route.request().url())
      if (url.pathname.endsWith("/last_verified")) {
        const limit = Number(url.searchParams.get("limit") ?? 25)
        const offset = Number(url.searchParams.get("offset") ?? 0)
        await route.fulfill({
          json: {
            items: VERIFIED_ITEMS.slice(offset, offset + limit),
            total: VERIFIED_ITEMS.length,
          },
        })
        return
      }

      if (url.pathname.endsWith("/verification/source")) {
        const codeHash = url.searchParams.get("code_hash") ?? VERIFIED_ITEMS[0].code_hash
        await route.fulfill({
          json: {
            code_hash: codeHash,
            verified: false,
            bundle: null,
          },
        })
        return
      }

      await route.abort()
    })
  })

  test("restores the page and scroll position after opening a contract", async ({page}) => {
    await page.goto("/verified")
    await page.getByRole("button", {name: "Go to page 2"}).click()
    await expect(page).toHaveURL(/\/verified\?page=2$/)

    const target = VERIFIED_ITEMS.at(44)
    if (!target) {
      throw new Error("Expected the target verified contract fixture")
    }
    const targetRow = page.getByRole("link", {name: `Open code hash ${target.code_hash}`})
    await targetRow.scrollIntoViewIfNeeded()
    await expect(targetRow).toBeVisible()
    const previousScrollY = await page.evaluate(() => window.scrollY)
    expect(previousScrollY).toBeGreaterThan(0)

    await targetRow.click()
    await expect(page).toHaveURL(`/verified/${target.code_hash}`)

    await page.goBack()

    await expect(page).toHaveURL(/\/verified\?page=2$/)
    await expect(page.getByRole("button", {name: "Go to page 2"})).toHaveAttribute(
      "aria-current",
      "page",
    )
    await expect(targetRow).toBeVisible()
    await expect
      .poll(async () => Math.abs((await page.evaluate(() => window.scrollY)) - previousScrollY))
      .toBeLessThan(2)
  })

  test("opens a code hash in a new tab with the middle mouse button", async ({context, page}) => {
    await page.goto("/verified")

    const target = VERIFIED_ITEMS.at(0)
    if (!target) {
      throw new Error("Expected a verified contract fixture")
    }
    const targetLink = page.getByRole("link", {name: `Open code hash ${target.code_hash}`})
    const newTabPromise = context.waitForEvent("page")
    await targetLink.click({button: "middle"})
    const newTab = await newTabPromise

    await expect(newTab).toHaveURL(`/verified/${target.code_hash}`)
    await expect(page).toHaveURL("/verified")
    await newTab.close()
  })

  test("opens a contract row in a new tab with the middle mouse button", async ({
    context,
    page,
  }) => {
    await page.goto("/verified")

    const target = VERIFIED_ITEMS.at(0)
    if (!target) {
      throw new Error("Expected a verified contract fixture")
    }

    const newTabPromise = context.waitForEvent("page")
    await page
      .getByRole("link", {name: `Open verified contract ${target.code_hash}`})
      .click({button: "middle"})
    const newTab = await newTabPromise

    await expect(newTab).toHaveURL(`/verified/${target.code_hash}`)
    await expect(page).toHaveURL("/verified")
    await newTab.close()
  })
})
