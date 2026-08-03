import {expect, test, type Page} from "@playwright/test"

import {prepareVisualPage} from "./visual"

const EXTERNAL_MESSAGE_BOC =
  "te6cckEBAQEAJQAARYgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEuN2rAw=="
const SOURCE_ADDRESS = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c"
const TARGET_ADDRESS = "EQAREREREREREREREREREREREREREREREREREREREREREeYT"
const SENT_TRANSACTION_HASH = "ab".repeat(32)

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
      const sendButton = getSendToLocalnetButton(page)
      const shareButton = getShareButton(page)

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
      if (app === "localnet") {
        await expect(sendButton).toBeVisible()
        await expect(sendButton).toBeDisabled()
        await expect(shareButton).toHaveCount(0)
      } else {
        await expect(sendButton).toHaveCount(0)
        await expect(shareButton).toBeVisible()
        await expect(shareButton).toBeDisabled()
      }

      await rawTab.click()
      await expect(rawTab).toHaveAttribute("aria-selected", "true")
      await expect(page.getByRole("textbox", {name: "Message BOC", exact: true})).toHaveAttribute(
        "placeholder",
        "Hex or base64 message BoC",
      )
      await expect(emulateButton).toBeDisabled()

      await page.getByRole("textbox", {name: "Message BOC", exact: true}).fill(EXTERNAL_MESSAGE_BOC)
      await expect(emulateButton).toBeEnabled()
      if (app === "localnet") {
        await expect(sendButton).toBeEnabled()
      } else {
        await expect(shareButton).toBeEnabled()
      }

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

    if (app === "explorer") {
      test("creates a Cloudflare share for the pinned emulation request", async ({page}) => {
        let requestBody: unknown
        await page.route("**/api/toncenter/*/v3/blocks?*", async interceptedRoute => {
          await interceptedRoute.fulfill({
            json: {
              blocks: [{workchain: -1, shard: "-9223372036854775808", seqno: 42, gen_utime: 1000}],
            },
          })
        })
        await page.route("**/api/emulations", async interceptedRoute => {
          requestBody = interceptedRoute.request().postDataJSON()
          await interceptedRoute.fulfill({
            status: 201,
            json: {
              id: "00000000-0000-4000-8000-000000000000",
              expiresAt: Date.now() + 30 * 24 * 60 * 60 * 1000,
            },
          })
        })

        await selectRawMessage(page, EXTERNAL_MESSAGE_BOC)
        await getShareButton(page).click()

        await expect(page.getByText("Share link copied", {exact: true})).toBeVisible()
        expect(requestBody).toMatchInlineSnapshot(`
          {
            "input": {
              "bounce": true,
              "inputMode": "raw",
              "mcSeqnoInput": "42",
              "messageTransport": "internal",
              "messageValue": "0.5",
              "rawMessage": "te6cckEBAQEAJQAARYgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEuN2rAw==",
              "sourceAddress": "",
              "targetAddress": "",
            },
            "options": {
              "ignoreChksig": false,
            },
            "version": 1,
          }
        `)
      })

      test("loads a shared request without starting its emulation", async ({page}) => {
        let shareRequestCount = 0
        let releaseTonApiRequests = () => {}
        const tonApiRequestGate = new Promise<void>(resolve => {
          releaseTonApiRequests = resolve
        })
        await page.route(
          "**/api/emulations/00000000-0000-4000-8000-000000000000",
          async interceptedRoute => {
            shareRequestCount += 1
            await interceptedRoute.fulfill({
              json: {
                emulation: {
                  version: 1,
                  input: {
                    inputMode: "raw",
                    targetAddress: "",
                    sourceAddress: "",
                    messageValue: "0.5",
                    messageTransport: "internal",
                    bounce: true,
                    mcSeqnoInput: "42",
                    rawMessage: EXTERNAL_MESSAGE_BOC,
                  },
                  options: {
                    ignoreChksig: true,
                    now: 1000,
                  },
                },
                expiresAt: Date.now() + 30 * 24 * 60 * 60 * 1000,
              },
            })
          },
        )
        await page.route(isTonApiRequest, async interceptedRoute => {
          await tonApiRequestGate
          await interceptedRoute.abort("connectionfailed")
        })

        await page.goto(`${route}?share=00000000-0000-4000-8000-000000000000`)

        await expect(page.getByRole("tab", {name: "Raw", exact: true})).toHaveAttribute(
          "aria-selected",
          "true",
        )
        await expect(page.getByRole("textbox", {name: "Message BOC", exact: true})).toHaveValue(
          EXTERNAL_MESSAGE_BOC,
        )
        await page.locator('summary[aria-label="Advanced options"]').click()
        await expect(
          page.getByRole("textbox", {name: "Masterchain block", exact: true}),
        ).toHaveValue("42")
        await expect(page.getByRole("checkbox", {name: /^Ignore CHKSIG/})).toBeChecked()
        await expect(getEmulateButton(page)).toBeEnabled()
        await expect(getEmulateButton(page)).not.toHaveAttribute("aria-busy", "true")
        await expect(page.getByRole("region", {name: "Emulation result"})).toHaveCount(0)
        expect(shareRequestCount).toBe(1)
        releaseTonApiRequests()
      })

      test("restores a shared builder through the emulation handoff payload", async ({page}) => {
        let sharedEmulation: Record<string, unknown> | undefined
        await page.route("**/api/toncenter/*/v3/blocks?*", async interceptedRoute => {
          await interceptedRoute.fulfill({
            json: {
              blocks: [{workchain: -1, shard: "-9223372036854775808", seqno: 42, gen_utime: 1000}],
            },
          })
        })
        await page.route("**/api/emulations", async interceptedRoute => {
          sharedEmulation = interceptedRoute.request().postDataJSON() as Record<string, unknown>
          await interceptedRoute.fulfill({
            status: 201,
            json: {
              id: "00000000-0000-4000-8000-000000000000",
              expiresAt: Date.now() + 30 * 24 * 60 * 60 * 1000,
            },
          })
        })

        await page.getByRole("combobox", {name: "From", exact: true}).fill(SOURCE_ADDRESS)
        await page.getByRole("combobox", {name: "To", exact: true}).fill(TARGET_ADDRESS)
        await getShareButton(page).click()
        await expect(page.getByText("Share link copied", {exact: true})).toBeVisible()
        expect(sharedEmulation).toMatchObject({
          version: 1,
          input: {
            inputMode: "builder",
            targetAddress: TARGET_ADDRESS,
            sourceAddress: SOURCE_ADDRESS,
            messageValue: "0.5",
            messageTransport: "internal",
            bounce: true,
            mcSeqnoInput: "42",
            rawMessage: expect.any(String),
            builder: {
              abiSourceMode: "auto",
              abiEndpoint: "destination",
              messageName: "empty",
              argsJson: "{}",
            },
          },
          options: {
            ignoreChksig: false,
          },
        })

        await page.route(
          "**/api/emulations/00000000-0000-4000-8000-000000000000",
          async interceptedRoute => {
            await interceptedRoute.fulfill({
              json: {
                emulation: sharedEmulation,
                expiresAt: Date.now() + 30 * 24 * 60 * 60 * 1000,
              },
            })
          },
        )
        await page.route(isTonApiRequest, async interceptedRoute => {
          await interceptedRoute.abort("connectionfailed")
        })

        await page.goto(`${route}?share=00000000-0000-4000-8000-000000000000`)

        await expect(page.getByRole("tab", {name: "Builder", exact: true})).toHaveAttribute(
          "aria-selected",
          "true",
        )
        await expect(page.getByRole("combobox", {name: "From", exact: true})).toHaveValue(
          SOURCE_ADDRESS,
        )
        await expect(page.getByRole("combobox", {name: "To", exact: true})).toHaveValue(
          TARGET_ADDRESS,
        )
        await expect(page.getByRole("textbox", {name: "Value", exact: true})).toHaveValue("0.5")
        await expect(page.getByRole("textbox", {name: "Message BOC", exact: true})).toHaveCount(0)
      })
    }

    if (app === "localnet") {
      test("sends a builder message through the localnet internal-message endpoint", async ({
        page,
      }) => {
        let requestBody: unknown
        await mockSentTransactionTrace(page)
        await page.route("**/acton_sendInternalMessage", async interceptedRoute => {
          requestBody = interceptedRoute.request().postDataJSON()
          await interceptedRoute.fulfill({
            json: {ok: true, result: {"@type": "ok", hash: "internal-message-hash"}},
          })
        })

        await page.getByRole("combobox", {name: "From", exact: true}).fill(SOURCE_ADDRESS)
        await page.getByRole("combobox", {name: "To", exact: true}).fill(TARGET_ADDRESS)

        const sendButton = getSendToLocalnetButton(page)
        await expect(sendButton).toBeEnabled()
        await sendButton.click()

        await expect.poll(() => requestBody).toMatchObject({boc: expect.any(String)})
        await expect(page.getByText("Message sent to localnet", {exact: true})).toBeVisible()
        await expect(
          page.getByRole("link", {name: "View transaction", exact: true}),
        ).toHaveAttribute("href", `/explorer/tx/${SENT_TRANSACTION_HASH}`)
      })

      test("requires confirmation before sending while account overrides are configured", async ({
        page,
      }) => {
        let sendRequestCount = 0
        await mockSentTransactionTrace(page)
        await page.route("**/api/v2/sendBocReturnHash", async interceptedRoute => {
          sendRequestCount += 1
          await interceptedRoute.fulfill({
            json: {
              ok: true,
              result: {"@type": "raw.extMessageInfo", hash: "external-message-hash"},
            },
          })
        })

        await selectRawMessage(page, EXTERNAL_MESSAGE_BOC)
        await page.locator('summary[aria-label="State overrides"]').click()
        await page.getByRole("button", {name: "Add account", exact: true}).click()

        await getSendToLocalnetButton(page).click()

        await expect(
          page.getByRole("dialog", {name: "Send without account overrides?"}),
        ).toBeVisible()
        expect(sendRequestCount).toBe(0)
        await expect(page.getByText(/current localnet state/)).toBeVisible()

        await page.getByRole("button", {name: "Send without overrides", exact: true}).click()
        await expect.poll(() => sendRequestCount).toBe(1)
        await expect(page.getByText("Message sent to localnet", {exact: true})).toBeVisible()
      })
    }
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

function getSendToLocalnetButton(page: Page) {
  return page.getByRole("tabpanel").getByRole("button", {
    name: "Send to localnet",
    exact: true,
  })
}

function getShareButton(page: Page) {
  return page.getByRole("tabpanel").getByRole("button", {
    name: "Share",
    exact: true,
  })
}

async function mockSentTransactionTrace(page: Page): Promise<void> {
  await page.route("**/api/v3/traces?*", async interceptedRoute => {
    await interceptedRoute.fulfill({
      json: {
        address_book: {},
        metadata: {},
        traces: [
          {
            trace: {tx_hash: SENT_TRANSACTION_HASH},
            transactions_order: [],
          },
        ],
      },
    })
  })
}
