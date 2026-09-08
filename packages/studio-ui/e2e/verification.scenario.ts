// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import type {Page} from "@playwright/test"

/** Exercises the real Studio shell while every publication and wallet request stays local */
export async function verificationScenario(page: Page) {
  const address = `0:${"45".repeat(32)}`
  const paymentAddress = "0QBFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRY6Q"
  const codeHash = "22".repeat(32)
  let matches = true
  let verified = false
  let statusHashOverride: string | undefined
  let otherCodeHash = codeHash
  let previews = 0
  let lastPreviewId = ""
  const previewHashes = new Map<string, string>()
  const expired = new Set<string>()
  const otherAddress = `0:${"46".repeat(32)}`
  let failPreview = false
  let preparations = 0
  let publications = 0
  let failUpload = false
  let paymentHash = ""
  const paymentNetworks: string[] = []
  let walletRequests = 0
  let operationPolls = 0
  let pausedPath: string | undefined
  let releaseRequest: (() => void) | undefined

  await page.unrouteAll({behavior: "ignoreErrors"})
  await page.route("**/api/v1/**", async route => {
    const path = route
      .request()
      .url()
      .split("?")[0]
      .replace(/^https?:\/\/[^/]+/, "")
    if (pausedPath && path.endsWith(pausedPath)) {
      await new Promise<void>(resolve => {
        releaseRequest = resolve
      })
    }
    const network = path.includes("/mainnet/") ? "mainnet" : "testnet"
    const requestedHash = path.endsWith("/verification/preview")
      ? route.request().postDataJSON().codeHash
      : decodeURIComponent(
          route
            .request()
            .url()
            .match(/[?&]codeHash=([^&]+)/)?.[1] ?? codeHash,
        )
    const status = {
      codeHash: statusHashOverride ?? requestedHash,
      verified,
      verifierUrl: `https://verifier.acton.monster/${codeHash}`,
    }
    let body: unknown = []

    if (path.endsWith("/events")) {
      await route.fulfill({contentType: "text/event-stream", body: ""})
      return
    }

    if (path.endsWith("/info"))
      body = {protocolVersion: 1, serverVersion: "test", workspace: {name: "Counter"}}
    if (path === "/api/v1/environments")
      body = ["testnet", "mainnet"].map(id => ({
        id,
        name: id === "testnet" ? "Testnet" : "Mainnet",
        status: "running",
        lifecycle: "external",
        rpcUrl: `/api/v1/environments/${id}/rpc`,
        config: {kind: "remoteTonNetwork", network: id},
        capabilities: ["explorer", "contracts", "wallets"],
        endpoints: {
          apiV2: `/api/v1/environments/${id}/rpc/api/v2`,
          apiV3: `/api/v1/environments/${id}/rpc/api/v3`,
        },
        network: {
          id,
          label: id === "testnet" ? "Testnet" : "Mainnet",
          chainId: id === "testnet" ? -3 : -239,
          testOnly: id === "testnet",
          supportsActions: true,
        },
      }))
    if (path.endsWith("/wallets"))
      body = [
        {
          name: "deployer",
          address: "EQC7mW7YSE93LQ7jZk3g4O9NYJ9KClLVFn4EKe3yYf4WZ4fN",
          publicKey: `0x${"12".repeat(32)}`,
          version: "v5r1",
          walletId: 2_147_483_405,
          workchain: 0,
        },
      ]
    if (path.endsWith("/acton_listContracts"))
      body = [
        {address, codeHash, name: "Counter", status: "active", sourceKind: "network"},
        {
          address: otherAddress,
          codeHash: otherCodeHash,
          name: "Other",
          status: "active",
          sourceKind: "network",
        },
      ]
    if (path.endsWith("/verification/status")) body = status
    if (path.endsWith("/verification/preview")) {
      if (failPreview) {
        await route.fulfill({
          status: 502,
          json: {error: {message: "The source registry is unavailable"}},
        })
        return
      }
      if (route.request().postDataJSON().address)
        throw new Error("Verification preview is tied to an address")
      lastPreviewId = `preview-${++previews}`
      previewHashes.set(lastPreviewId, requestedHash)
      body = {
        id: lastPreviewId,
        status,
        compilerVersion: "1.4.2",
        payment: {
          address: paymentAddress,
          amount: "12340000",
          network: "testnet",
          comment: `acton-verify:v1:${codeHash}`,
        },
        candidates: [
          {
            contractId: "Counter",
            sourcePath: "contracts/counter.tolk",
            codeHash: requestedHash,
            matches,
            error: null,
            files: [
              {path: "contracts/counter.tolk", sizeBytes: 2056},
              {path: "contracts/storage.tolk", sizeBytes: 715},
            ],
          },
        ],
      }
    }
    const operationId = path.endsWith("/verification/operations")
      ? route.request().postDataJSON().previewId
      : path.match(/verification\/operations\/([^/]+)/)?.[1]
    if (operationId && expired.has(operationId)) {
      await route.fulfill({
        status: 409,
        json: {
          error: {
            code: "verification_preview_expired",
            message: "This verification preview expired; check the sources again",
          },
        },
      })
      return
    }
    if (path.endsWith("/verification/operations")) {
      if (!previewHashes.has(operationId)) throw new Error("Unknown preview")
      preparations += 1
      body = {
        id: operationId,
        phase: "ready",
        message: {address: paymentAddress, amount: "12340000", payload: "te6ccgEBAQEAAgAAAA=="},
        error: null,
      }
    }
    if (operationId && path.endsWith("/payment")) {
      const receivedHash = route.request().postDataJSON().messageHash
      if (paymentHash && receivedHash !== paymentHash) throw new Error("Retry changed the payment")
      paymentHash = receivedHash
      publications += 1
      operationPolls = 0
      body = {id: operationId, phase: "confirmingPayment", message: null, error: null}
    }
    if (operationId && !path.endsWith("/payment") && !path.endsWith("/verification/operations")) {
      operationPolls += 1
      verified = operationPolls > 1 && !failUpload
      body = {
        id: operationId,
        phase: operationPolls === 1 ? "uploadingSources" : failUpload ? "failed" : "verified",
        message: null,
        error: failUpload ? "Source storage is temporarily unavailable" : null,
      }
    }
    if (path.endsWith("/sign")) {
      walletRequests += 1
      paymentNetworks.push(network)
      body = {signature: `0x${"00".repeat(64)}`}
    }
    if (path.endsWith("/api/v3/message")) {
      walletRequests += 1
      paymentNetworks.push(network)
      body = {message_hash_norm: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}
    }
    if (path.endsWith("/getAddressInformation"))
      body = {ok: true, result: {state: "active", balance: "1000000000", code: "", data: ""}}
    if (path.endsWith("/addressInformation")) {
      body = {
        status: "active",
        balance: "1000000000",
        code: "",
        data: "",
        last_transaction_hash: "00".repeat(32),
        last_transaction_lt: "1",
      }
    }
    if (path.endsWith("/runGetMethod"))
      body = {ok: true, result: {exit_code: 0, stack: [["num", "0x0"]]}}

    await route.fulfill({json: body})
  })

  const observations: string[] = []
  const notifications = page.getByRole("region", {name: "Notifications"})
  const dialog = page.getByRole("dialog", {name: "Verify contract source", exact: true})
  const open = page.getByRole("button", {name: "Verify source", exact: true})
  const publish = dialog.getByRole("button", {name: "Continue", exact: true})
  const close = dialog.getByRole("button", {name: "Close", exact: true})

  for (const network of ["testnet", "mainnet"]) {
    await page.goto(`/networks/${network}/contracts/${address}`)
    if (network === "testnet") {
      await page.evaluate(() => {
        for (const key of Object.keys(sessionStorage)) {
          if (key.startsWith("acton:verification:")) sessionStorage.removeItem(key)
        }
      })
      await page.reload()
    }
    await open.click()
    await dialog.getByText("Compiled code matches the requested hash").waitFor()
    await publish.waitFor({state: "visible"})
    await page.waitForFunction(() =>
      [...document.querySelectorAll("button")].some(
        button => button.textContent === "Continue" && !button.disabled,
      ),
    )
    if (publications !== 0 || preparations !== (network === "testnet" ? 0 : 2))
      throw new Error("Preview uploaded files without approval")

    await publish.click()
    await dialog.getByRole("button", {name: "Pay and verify", exact: true}).waitFor()
    await close.click()
    await open.click()
    await dialog.getByRole("button", {name: "Pay and verify", exact: true}).waitFor()
    if (walletRequests !== 0)
      throw new Error("Preparation used the wallet before payment confirmation")
    observations.push(
      `${network}: reviewed sources, prepared once, no wallet request, ready after dismissal`,
    )

    statusHashOverride = "33".repeat(32)
    await dialog.getByRole("button", {name: "Pay and verify", exact: true}).click()
    await page
      .getByRole("region", {name: "Notifications"})
      .getByText("The verifier returned a different code hash; payment has not been sent")
      .waitFor()
    if (walletRequests !== 0) throw new Error("Code change did not block signing")
    observations.push(`${network}: unexpected registry hash blocks signing`)
    statusHashOverride = undefined
    await dialog.getByRole("button", {name: "Check sources again", exact: true}).click()
    await publish.click()
    await dialog.getByRole("button", {name: "Pay and verify", exact: true}).waitFor()

    verified = true
    await dialog.getByRole("button", {name: "Pay and verify", exact: true}).click()
    await notifications.getByText("Source verified", {exact: true}).waitFor()
    if (walletRequests !== 0) throw new Error("Existing proof triggered another payment")
    observations.push(`${network}: proof published meanwhile skips payment`)
    verified = false
  }

  // Exercise signing and broadcast through the existing wallet adapter, then retry after reload.
  failUpload = true
  await page.reload()
  await open.click()
  await publish.click()
  pausedPath = "/sign"
  await dialog.getByRole("button", {name: "Pay and verify", exact: true}).click()
  await notifications.getByText("Signing Testnet payment", {exact: true}).waitFor()
  await dialog.waitFor({state: "visible"})

  pausedPath = "/api/v3/message"
  releaseRequest?.()
  await notifications.getByText("Sending Testnet payment", {exact: true}).waitFor()
  pausedPath = "/payment"
  releaseRequest?.()
  await notifications.getByText("Waiting for finalized Testnet payment", {exact: true}).waitFor()
  const progressToast = await notifications.locator('[data-variant="loading"]').elementHandle()
  pausedPath = undefined
  releaseRequest?.()
  await notifications.getByText("Uploading sources", {exact: true}).waitFor()
  if (
    !(await progressToast?.evaluate(element => element.textContent?.includes("Uploading sources")))
  ) {
    throw new Error("Progress creates a new toast instead of updating it")
  }
  await notifications.getByText("Retry verification", {exact: true}).waitFor()
  if (
    !(await progressToast?.evaluate(element => element.getAttribute("data-variant") === "error"))
  ) {
    throw new Error("Error does not replace the progress toast")
  }
  observations.push(
    "progress: signing, sending, finality, upload and error update one toast, dialog stays open",
  )
  if (
    walletRequests !== 2 ||
    publications !== 1 ||
    paymentNetworks.some(network => network !== "testnet")
  ) {
    throw new Error(
      `Mainnet verification used the wrong payment flow: ${walletRequests}, ${publications}, ${paymentNetworks}`,
    )
  }
  observations.push("mainnet: dynamic Testnet payment precedes source publication")

  const paidPreviewId = lastPreviewId
  await close.click()
  await page.goto(`/networks/mainnet/contracts/${otherAddress}`)
  await open.click()
  await dialog.getByRole("button", {name: "Retry verification", exact: true}).waitFor()
  if (lastPreviewId !== paidPreviewId)
    throw new Error("Another instance of the same code starts another attempt")
  observations.push("same code hash: another instance resumes the existing attempt")
  await close.click()

  otherCodeHash = "55".repeat(32)
  await page.reload()
  await open.click()
  await publish.waitFor()
  if (lastPreviewId === paidPreviewId)
    throw new Error("Different code hash opened the previous attempt")
  await close.click()
  await page.goto(`/networks/mainnet/contracts/${address}`)
  await open.click()
  await dialog.getByRole("button", {name: "Retry verification", exact: true}).waitFor()
  observations.push("different code hash: independent source review preserves the original payment")

  expired.add(paidPreviewId)
  await dialog.getByRole("button", {name: "Retry verification", exact: true}).click()
  await dialog.getByRole("button", {name: "Check sources again", exact: true}).waitFor()
  await notifications
    .getByText("This verification preview expired; check the sources again", {exact: true})
    .waitFor()
  if (walletRequests !== 2) throw new Error("Expired attempt requests another payment")
  await dialog.getByRole("button", {name: "Check sources again", exact: true}).click()
  await dialog.getByRole("button", {name: "Resume verification", exact: true}).waitFor()
  if (lastPreviewId === paidPreviewId) throw new Error("Expired preview was not replaced")
  if (await dialog.getByRole("button", {name: "Pay and verify", exact: true}).count())
    throw new Error("Recovery offers another payment")
  observations.push("expired paid preview: dialog stays open, sources can be reviewed again")

  await page.reload()
  await open.click()
  failUpload = false
  await dialog.getByRole("button", {name: "Resume verification", exact: true}).click()
  await notifications.getByText("Source verified", {exact: true}).waitFor()
  if (walletRequests !== 2 || publications !== 2) throw new Error("Retry paid again")
  observations.push(
    "reload: renewed source review resumes with the same payment, no second signing or broadcast",
  )
  verified = false

  await page.reload()
  await open.click()
  await publish.waitFor()
  expired.add(lastPreviewId)
  await publish.click()
  await dialog.getByRole("button", {name: "Check sources again", exact: true}).waitFor()
  await dialog.getByRole("button", {name: "Check sources again", exact: true}).click()
  await publish.click()
  await dialog.getByRole("button", {name: "Pay and verify", exact: true}).waitFor()
  if (walletRequests !== 2) throw new Error("Unpaid expiration used the wallet")
  await close.click()
  observations.push("expired unpaid preview: new source review restores the payment confirmation")

  // A fresh page with a mismatched build must not allow publication.
  matches = false
  await page.reload()
  await open.click()
  await dialog.getByText("Code does not match", {exact: false}).waitFor()
  if (!(await publish.isDisabled())) throw new Error("Mismatched code can be published")
  observations.push("mismatch: publication disabled")
  await close.click()

  failPreview = true
  await page.reload()
  await open.click()
  await page
    .getByRole("region", {name: "Notifications"})
    .getByText("The source registry is unavailable")
    .waitFor()
  await dialog.getByRole("button", {name: "Check sources again", exact: true}).waitFor()
  if (await dialog.getByText("The source registry is unavailable", {exact: true}).count())
    throw new Error("API error rendered inside dialog")
  observations.push("registry error: toast, dialog stays open with source review retry")

  verified = true
  failPreview = false
  await page.reload()
  await page.getByRole("link", {name: "Source verified", exact: true}).waitFor()
  if (await open.count()) throw new Error("Verified contract still offers publication")
  observations.push("public proof: verified link replaces publication")
  return `${observations.join("\n")}\n`
}
