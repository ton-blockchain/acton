import {expect, test, type Page} from "@playwright/test"

import {prepareVisualPage} from "./visual"

const EXTERNAL_MESSAGE_BOC =
  "te6cckEBAQEAJQAARYgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEuN2rAw=="

interface EmulateSuiteOptions {
  readonly app: "explorer" | "localnet"
  readonly route: string
}

export function describeEmulatePage({app, route}: EmulateSuiteOptions): void {
  test.describe("Emulate Transaction", () => {
    test.beforeEach(async ({page}) => {
      await prepareVisualPage(page, {app})
      await page.goto(route)
      await expect(
        page.getByRole("heading", {name: "Emulate Transaction", exact: true}),
      ).toBeVisible()
    })

    test("opens the builder and switches to raw BOC input", async ({page}) => {
      const builderTab = page.getByRole("tab", {name: "Builder", exact: true})
      const rawTab = page.getByRole("tab", {name: "Raw", exact: true})
      const emulateButton = getEmulateButton(page)

      await expect(builderTab).toHaveAttribute("aria-selected", "true")
      await expect(page.getByRole("combobox", {name: "From", exact: true})).toBeVisible()
      await expect(page.getByRole("combobox", {name: "To", exact: true})).toBeVisible()
      await expect(
        page.getByText("Enter a valid contract address in To to configure the message", {
          exact: true,
        }),
      ).toBeVisible()
      await expect(page.getByText("ABI not loaded", {exact: true})).toBeHidden()
      await expect(emulateButton).toBeDisabled()

      await rawTab.click()
      await expect(rawTab).toHaveAttribute("aria-selected", "true")
      await expect(page.getByRole("textbox", {name: "Message BOC", exact: true})).toHaveAttribute(
        "placeholder",
        "Hex or base64 message BoC",
      )
      await expect(emulateButton).toBeDisabled()

      await page.getByRole("textbox", {name: "Message BOC", exact: true}).fill(EXTERNAL_MESSAGE_BOC)
      await expect(emulateButton).toBeEnabled()

      const resetButton = page.getByRole("button", {name: "Reset transaction fields", exact: true})
      await expect(resetButton).toHaveAttribute(
        "title",
        "Reset all transaction fields to their default values",
      )
      await resetButton.click()
      await expect(rawTab).toHaveAttribute("aria-selected", "true")
      await expect(page.getByRole("textbox", {name: "Message BOC", exact: true})).toHaveValue("")
      await expect(emulateButton).toBeDisabled()
    })

    test("restores and persists transaction options in the URL", async ({page}) => {
      const params = new URLSearchParams({
        source: "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c",
        address: "EQAREREREREREREREREREREREREREREREREREREREREREeYT",
        value: "2",
        bounce: "false",
        mcSeqno: "7",
        ignoreChksig: "true",
        timeMode: "increase",
        increaseTime: "15",
        timestamp: "1234567890",
      })
      await page.goto(`${route}?${params}`)

      await expect(page.getByRole("combobox", {name: "From", exact: true})).toHaveValue(
        params.get("source") ?? "",
      )
      await expect(page.getByRole("combobox", {name: "To", exact: true})).toHaveValue(
        params.get("address") ?? "",
      )
      await expect(page.getByRole("textbox", {name: "Value", exact: true})).toHaveValue("2")

      await page.locator('summary[aria-label="Advanced options"]').click()
      const bounce = page.getByRole("checkbox", {name: /^Bounce/})
      const ignoreChksig = page.getByRole("checkbox", {name: /^Ignore CHKSIG/})
      const block = page.getByRole("textbox", {name: "Masterchain block", exact: true})
      await expect(bounce).not.toBeChecked()
      await expect(ignoreChksig).toBeChecked()
      await expect(block).toHaveValue("7")

      await page.locator('summary[aria-label="Override timestamp"]').click()
      const increaseMode = page.getByRole("radio", {name: "Increase time", exact: true})
      const timestampMode = page.getByRole("radio", {name: "Set UNIX timestamp", exact: true})
      await expect(increaseMode).toBeChecked()
      const seconds = page.getByRole("textbox", {name: "Seconds to add", exact: true})
      await expect(seconds).toHaveValue("15")

      await timestampMode.check()
      const unixTimestamp = page.getByRole("textbox", {name: "UNIX timestamp", exact: true})
      await expect(unixTimestamp).not.toHaveValue("")
      const currentTimestamp = await unixTimestamp.inputValue()
      expect(currentTimestamp).not.toBe("1234567890")
      await increaseMode.check()
      await expect(seconds).toHaveValue("15")

      await bounce.check()
      await ignoreChksig.uncheck()
      await block.fill("8")
      await seconds.fill("30")

      await expect
        .poll(() => Object.fromEntries(new URL(page.url()).searchParams))
        .toMatchObject({
          source: params.get("source"),
          address: params.get("address"),
          value: "2",
          mcSeqno: "8",
          timeMode: "increase",
          increaseTime: "30",
          timestamp: currentTimestamp,
        })
      await expect.poll(() => new URL(page.url()).searchParams.has("bounce")).toBe(false)
      await expect.poll(() => new URL(page.url()).searchParams.has("ignoreChksig")).toBe(false)
    })

    test("reports malformed raw BOC input", async ({page}) => {
      await selectRawMessage(page, "not a BOC")
      await getEmulateButton(page).click()

      await expect(
        page
          .getByRole("alert")
          .filter({hasText: "Message BOC must be encoded as hex or base64"})
          .last(),
      ).toBeVisible()
    })

    test("shows loading and recovers from an API failure", async ({page}) => {
      let releaseRequests = () => {}
      const requestGate = new Promise<void>(resolve => {
        releaseRequests = resolve
      })

      await page.route(isTonApiRequest, async interceptedRoute => {
        await requestGate
        await interceptedRoute.abort("connectionfailed")
      })

      await selectRawMessage(page, EXTERNAL_MESSAGE_BOC)
      const emulateButton = getEmulateButton(page)
      await emulateButton.click()

      await expect(emulateButton).toHaveAttribute("aria-busy", "true")
      await expect(emulateButton).toBeDisabled()
      releaseRequests()

      await expect(page.getByRole("alert").last()).toBeVisible()
      await expect(emulateButton).not.toHaveAttribute("aria-busy", "true")
      await expect(emulateButton).toBeEnabled()
    })
  })
}

async function selectRawMessage(page: Page, boc: string): Promise<void> {
  await page.getByRole("tab", {name: "Raw", exact: true}).click()
  await page.getByRole("textbox", {name: "Message BOC", exact: true}).fill(boc)
}

function isTonApiRequest(url: URL): boolean {
  return (
    url.hostname === "toncenter.com" ||
    url.pathname.startsWith("/api/v2") ||
    url.pathname.startsWith("/api/v3")
  )
}

function getEmulateButton(page: Page) {
  return page.getByRole("tabpanel").getByRole("button", {name: "Emulate", exact: true})
}
