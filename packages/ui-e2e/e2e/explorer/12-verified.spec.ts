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

const PAYMENT_CODE_HASH = "5e3bd9ba054aa0c98c6d0244aa7e7f4e381a603ee022063a266db29dbb06e6dd"
const LEGACY_CODE_HASH = "7f3f78e3b8c68282b629bd8a6d49726f97902f4b36d9295d170a0e67ac9daa04"
const PAYMENT_TX_HASH = "a67afe0194b06c636a87ec5c5876d7d0c02579eb43755d7dc106647f08f89175"

const verifiedSourceResponse = (codeHash: string, paymentTransactionHash?: string) => ({
  code_hash: codeHash,
  verified: true,
  bundle: {
    source_bundle_hash: "4e3017a5b9b184a201d7e25490f1585ac1c2a1ee8b95f8478c72601481ac5a16",
    ...(paymentTransactionHash ? {payment_tx_hash: paymentTransactionHash} : {}),
    verified_at: 1_750_000_000,
    storage_revision: "263e28a9dbfffb7cd5a7cff7d70ceef28a9d1134",
    entrypoint: "contracts/JettonMinter.tolk",
    compiler: {
      language: "tolk",
      version: "1.4.2",
      params: {compiler_version: "1.4.2"},
    },
    files: [
      {
        path: "contracts/JettonMinter.tolk",
        content_hash: "1".repeat(64),
        include_in_command: null,
        is_stdlib: null,
        has_include_directives: null,
        content: "fun onInternalMessage() {}",
      },
    ],
  },
})

const VERIFIED_SOURCE_RESPONSES = new Map([
  [PAYMENT_CODE_HASH, verifiedSourceResponse(PAYMENT_CODE_HASH, PAYMENT_TX_HASH)],
  [LEGACY_CODE_HASH, verifiedSourceResponse(LEGACY_CODE_HASH)],
])

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
          json: VERIFIED_SOURCE_RESPONSES.get(codeHash) ?? {
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
    await expect(page).toHaveURL(/\/verified\?network=mainnet&page=2$/)

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
    await expect(page).toHaveURL(`/verified/${target.code_hash}?network=mainnet`)

    await page.goBack()

    await expect(page).toHaveURL(/\/verified\?network=mainnet&page=2$/)
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
    await expect(page).toHaveURL(/\/verified\?network=mainnet$/)
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
    await expect(page).toHaveURL(/\/verified\?network=mainnet$/)
    await newTab.close()
  })

  test("links a verification payment to Actonscan without a copy action", async ({page}) => {
    await page.goto(`/verified/${PAYMENT_CODE_HASH}`)

    const paymentLink = page.getByRole("link", {
      name: `View payment transaction ${PAYMENT_TX_HASH} on Actonscan`,
    })
    const shortenedHash = `${PAYMENT_TX_HASH.slice(0, 8)}…${PAYMENT_TX_HASH.slice(-8)}`

    await expect(page.getByText("Payment tx", {exact: true})).toBeVisible()
    await expect(paymentLink).toHaveAttribute(
      "href",
      `https://actonscan.com/tx/${PAYMENT_TX_HASH}?network=testnet`,
    )
    await expect(paymentLink).toHaveText(shortenedHash)
    await expect(page.getByRole("button", {name: /copy payment/i})).toHaveCount(0)
  })

  test("omits payment metadata for bundles without a payment transaction", async ({page}) => {
    await page.goto(`/verified/${LEGACY_CODE_HASH}`)

    await expect(page.getByText("Payment tx", {exact: true})).toHaveCount(0)
    await expect(page.getByRole("link", {name: /payment transaction/i})).toHaveCount(0)
  })

  test("keeps payment metadata inside a narrow viewport", async ({page}) => {
    await page.setViewportSize({width: 360, height: 800})
    await page.goto(`/verified/${PAYMENT_CODE_HASH}`)

    await expect(page.getByText("Payment tx", {exact: true})).toBeVisible()
    const paymentLink = page.getByRole("link", {
      name: `View payment transaction ${PAYMENT_TX_HASH} on Actonscan`,
    })
    const bounds = await paymentLink.boundingBox()
    expect(bounds).not.toBeNull()
    expect(bounds?.x ?? -1).toBeGreaterThanOrEqual(0)
    expect((bounds?.x ?? 0) + (bounds?.width ?? 361)).toBeLessThanOrEqual(360)
  })
})
