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
      const builderTab = page.getByRole("button", {name: "Builder", exact: true})
      const rawTab = page.getByRole("button", {name: "Raw BOC", exact: true})
      const emulateButton = page.getByRole("button", {name: "Emulate", exact: true})

      await expect(builderTab).toHaveAttribute("aria-pressed", "true")
      await expect(page.getByRole("textbox", {name: "Target contract", exact: true})).toBeVisible()
      await expect(emulateButton).toBeDisabled()

      await rawTab.click()
      await expect(rawTab).toHaveAttribute("aria-pressed", "true")
      await expect(page.getByRole("textbox", {name: "Message BOC", exact: true})).toBeVisible()
      await expect(emulateButton).toBeDisabled()

      await page.getByRole("textbox", {name: "Message BOC", exact: true}).fill(EXTERNAL_MESSAGE_BOC)
      await expect(emulateButton).toBeEnabled()

      await page.getByRole("button", {name: "Reset", exact: true}).click()
      await expect(builderTab).toHaveAttribute("aria-pressed", "true")
      await expect(emulateButton).toBeDisabled()
    })

    test("reports malformed raw BOC input", async ({page}) => {
      await selectRawMessage(page, "not a BOC")
      await page.getByRole("button", {name: "Emulate", exact: true}).click()

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
      await page.getByRole("button", {name: "Emulate", exact: true}).click()

      await expect(page.getByText("Emulating message", {exact: true})).toBeVisible()
      releaseRequests()

      await expect(page.getByText("Emulating message", {exact: true})).toBeHidden()
      await expect(page.getByRole("alert").last()).toBeVisible()
      await expect(page.getByRole("button", {name: "Emulate", exact: true})).toBeEnabled()
    })
  })
}

async function selectRawMessage(page: Page, boc: string): Promise<void> {
  await page.getByRole("button", {name: "Raw BOC", exact: true}).click()
  await page.getByRole("textbox", {name: "Message BOC", exact: true}).fill(boc)
}

function isTonApiRequest(url: URL): boolean {
  return (
    url.hostname === "toncenter.com" ||
    url.pathname.startsWith("/api/v2") ||
    url.pathname.startsWith("/api/v3")
  )
}
