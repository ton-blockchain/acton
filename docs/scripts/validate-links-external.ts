import {type ExternalLinkResult, printErrors, validateFiles} from "next-validate-link"
import {createLinkValidationConfig, getLinkValidationInput} from "./link-validation"

const externalLinkTimeoutMs = 15_000

async function validateExternalLinks() {
  const {files, scanned} = await getLinkValidationInput()

  printErrors(
    await validateFiles(
      files,
      createLinkValidationConfig(scanned, {
        checkRelativePaths: false,
        checkRelativeUrls: false,

        whitelist,
        checkExternal: {
          validate: validateExternalUrl,
        },
      }),
    ),
    true,
  )
}

function whitelist(url: string): boolean {
  // CDN return 502
  if (url.startsWith("https://cdn.tapps.ninja")) {
    return true
  }

  return false
}

async function validateExternalUrl(url: URL): Promise<ExternalLinkResult> {
  try {
    return await checkExternalUrl(url)
  } catch (error: unknown) {
    return {
      success: false,
      message: error instanceof Error ? error.message : String(error),
    }
  }
}

function toRedirectMode(url: URL): RequestRedirect {
  // The latest release can be accessed via a redirect from 'latest'
  if (url.href.startsWith("https://github.com/ton-blockchain/acton/releases/latest")) {
    return "follow"
  }

  return "manual"
}

function checkLocalhostPort(url: URL): ExternalLinkResult {
  if (url.port === "5173") {
    return {success: true}
  }

  return {
    success: false,
    message: `port ${url.port} is not allowed for localhost`,
  }
}

async function checkExternalUrl(url: URL): Promise<ExternalLinkResult> {
  if (url.hostname === "localhost") {
    return checkLocalhostPort(url)
  }

  const redirectMode = toRedirectMode(url)
  const response = await fetch(url, {
    method: "GET",
    redirect: redirectMode,
    signal: AbortSignal.timeout(externalLinkTimeoutMs),
  })

  await response.body?.cancel()

  const status = response.status
  if (status === 200) {
    return {success: true}
  }

  if (status >= 300 && status < 400) {
    return {
      success: false,
      message: `redirected to '${response.headers.get("location")}'`,
    }
  }

  if (status >= 400 && status < 500) {
    let message = `client error ${status}`
    switch (status) {
      case 404:
        message = `not found ${url}`
        break
    }

    return {
      success: false,
      message: message,
    }
  }

  return {
    success: false,
    message: `unknown response code ${status}`,
  }
}

void validateExternalLinks()
