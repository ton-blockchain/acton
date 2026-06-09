import {ApiReferenceReact, type AnyApiReferenceConfiguration} from "@scalar/api-reference-react"
import "@scalar/api-reference-react/style.css"
import * as React from "react"
import {useLocation} from "react-router-dom"

import styles from "./ApiReferencePage.module.css"

type ApiReferenceVersion = "control" | "v2" | "v3"

interface ApiReferencePageProps {
  readonly apiBaseUrl: string
  readonly theme: string
  readonly toncenterApiKey?: string
  readonly version: ApiReferenceVersion
}

const apiReferences: Record<
  ApiReferenceVersion,
  {
    readonly title: string
    readonly slug: string
    readonly specUrl: string
  }
> = {
  v2: {
    title: "TON Center API v2",
    slug: "ton-center-api-v2",
    specUrl: "/openapi/ton-api-v2.openapi.json",
  },
  v3: {
    title: "TON Center API v3",
    slug: "ton-center-api-v3",
    specUrl: "/openapi/ton-api-v3.openapi.json",
  },
  control: {
    title: "Acton Localnet Control API",
    slug: "acton-localnet-control-api",
    specUrl: "/openapi/acton-localnet-control.openapi.json",
  },
}

export const ApiReferencePage: React.FC<ApiReferencePageProps> = ({
  apiBaseUrl,
  theme,
  toncenterApiKey,
  version,
}) => {
  const reference = apiReferences[version]
  const localnetOrigin = React.useMemo(() => apiOrigin(apiBaseUrl), [apiBaseUrl])
  const syncReferenceAnchor = useApiReferenceAnchorSync(reference.slug)
  const toncenterFetch = React.useMemo(
    () => createToncenterApiFetch(apiBaseUrl, toncenterApiKey),
    [apiBaseUrl, toncenterApiKey],
  )
  const configuration = React.useMemo<AnyApiReferenceConfiguration>(
    () => ({
      title: reference.title,
      slug: reference.slug,
      url: reference.specUrl,
      authentication: toncenterApiKey
        ? {
            preferredSecurityScheme: "APIKeyHeader",
            securitySchemes: {
              APIKeyHeader: {
                value: toncenterApiKey,
              },
            },
          }
        : undefined,
      servers: [
        {
          url: localnetOrigin,
          description: "Acton localnet",
        },
      ],
      agent: {
        disabled: true,
      },
      baseServerURL: localnetOrigin,
      defaultHttpClient: {
        targetKey: "shell",
        clientKey: "curl",
      },
      fetch: toncenterFetch,
      customFetch: toncenterFetch,
      darkMode: theme === "dark",
      documentDownloadType: "json",
      forceDarkModeState: theme === "dark" ? "dark" : "light",
      hideClientButton: true,
      hideDarkModeToggle: true,
      hideSearch: true,
      layout: "modern",
      mcp: {
        disabled: true,
      },
      onLoaded: syncReferenceAnchor,
      showDeveloperTools: "never",
      telemetry: false,
      withDefaultFonts: false,
      customCss: `
        .scalar-app,
        .scalar-app .dark-mode,
        .scalar-app .light-mode {
          --scalar-font: var(--font-family);
          --scalar-font-code: ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", monospace;
          --scalar-radius: 4px;
          --scalar-radius-lg: 6px;
          --scalar-radius-xl: 8px;
          --scalar-background-1: var(--litenode-bg);
          --scalar-background-2: var(--litenode-surface);
          --scalar-background-3: var(--litenode-surface-subtle);
          --scalar-background-4: var(--litenode-control-hover);
          --scalar-background-accent: var(--litenode-accent-bg);
          --scalar-background-danger: var(--litenode-danger-bg);
          --scalar-color-1: var(--litenode-text);
          --scalar-color-2: var(--litenode-text-muted);
          --scalar-color-3: var(--litenode-text-faint);
          --scalar-color-accent: var(--tonscan-accent);
          --scalar-color-danger: var(--litenode-danger);
          --scalar-border-color: var(--litenode-border);
          --scalar-button-1: var(--litenode-primary);
          --scalar-button-1-color: var(--litenode-primary-foreground);
          --scalar-button-1-hover: var(--litenode-primary-hover);
          --scalar-link-color: var(--tonscan-link);
          --scalar-link-color-hover: var(--tonscan-link);
          --scalar-link-color-visited: var(--tonscan-link);
          --scalar-sidebar-background-1: var(--litenode-surface);
          --scalar-sidebar-border-color: var(--litenode-border);
          --scalar-sidebar-color-1: var(--litenode-text);
          --scalar-sidebar-color-2: var(--litenode-text-muted);
          --scalar-sidebar-color-active: var(--litenode-text);
          --scalar-sidebar-search-background: var(--litenode-surface-subtle);
          --scalar-sidebar-search-border-color: var(--litenode-border-control);
          --scalar-sidebar-search-color: var(--litenode-text-muted);
          --scalar-sidebar-item-hover-background: var(--litenode-surface-hover);
          --scalar-sidebar-item-hover-color: var(--litenode-text);
          --scalar-sidebar-item-active-background: var(--litenode-surface-active);
          --scalar-scrollbar-color: var(--litenode-border-strong);
          --scalar-scrollbar-color-active: var(--litenode-text-soft);
        }
        .scalar-app {
          background: var(--litenode-bg) !important;
          color: var(--litenode-text);
          letter-spacing: 0;
        }
        .scalar-app .references-layout {
          background: var(--litenode-bg) !important;
        }
        .scalar-app .t-doc__sidebar {
          background: var(--litenode-surface) !important;
          border-right: 1px solid var(--litenode-border) !important;
          color: var(--litenode-text);
          font-size: 14px;
        }
        .scalar-app .t-doc__sidebar button {
          min-height: 32px;
          border-radius: 8px;
          font-size: 0.84rem;
          line-height: 1.2;
        }
        .scalar-app .t-doc__sidebar button:hover {
          background: var(--litenode-surface-hover) !important;
          color: var(--litenode-text) !important;
        }
        .scalar-app .t-doc__sidebar .bg-sidebar-b-active {
          background: var(--litenode-surface-active) !important;
          color: var(--litenode-text-strong) !important;
        }
        .scalar-app .sidebar-heading-type {
          min-width: 34px;
          padding: 0;
          border: none !important;
          background: transparent !important;
          display: inline-flex;
          align-items: center;
          justify-content: flex-end;
          font-size: 0.62rem;
          font-weight: 700;
          letter-spacing: 0;
        }
        .scalar-app .scalar-card {
          border: 1px solid var(--litenode-border-control) !important;
          border-radius: 8px !important;
          background: var(--litenode-surface) !important;
          box-shadow: var(--litenode-inset-highlight);
        }
        .scalar-app .show-api-client-button {
          min-height: 26px;
          border: 1px solid var(--litenode-border-control) !important;
          border-radius: 6px;
          background: var(--litenode-surface-raised) !important;
          color: var(--litenode-text) !important;
          box-shadow: var(--litenode-inset-highlight);
        }
        .scalar-app .show-api-client-button:hover {
          background: var(--litenode-surface-hover) !important;
        }
        .scalar-app .show-api-client-button span,
        .scalar-app .show-api-client-button svg {
          color: inherit !important;
          fill: currentColor !important;
        }
        .scalar-app .client-libraries {
          color: var(--litenode-text-muted) !important;
          font-size: 0.82rem;
        }
        .scalar-app .client-libraries.rendered-code-sdks {
          margin-inline-end: 8px;
        }
        .scalar-app .client-libraries__active {
          color: var(--litenode-text) !important;
        }
        .scalar-app pre,
        .scalar-app code {
          font-size: 0.82rem;
        }
        .scalar-app [class*="rounded-xl"] {
          border-radius: 8px !important;
        }
        .scalar-app div:has(> a[href="https://www.scalar.com"]),
        .scalar-app div:has(> a[href="https://www.scalar.com/"]),
        .scalar-app a[href="https://www.scalar.com"],
        .scalar-app a[href="https://www.scalar.com/"],
        .scalar-app .darklight-reference,
        .scalar-app .response-section-content .flex-center:has(.scalar-version-number),
        .scalar-app .scalar-version-number {
          display: none !important;
        }
      `,
    }),
    [localnetOrigin, reference, syncReferenceAnchor, theme, toncenterApiKey, toncenterFetch],
  )

  return (
    <div className={styles.apiReferencePage}>
      <ApiReferenceReact configuration={configuration} />
    </div>
  )
}

interface ApiReferenceAnchorTarget {
  readonly path: string
  readonly hash: string
  readonly href: string
  readonly elementId: string
}

interface ApiReferenceAnchorLocation {
  readonly pathname: string
  readonly search: string
  readonly hash: string
}

function useApiReferenceAnchorSync(referenceSlug: string): () => void {
  const {hash, pathname, search} = useLocation()
  const targetRef = React.useRef<ApiReferenceAnchorTarget | undefined>(
    apiReferenceAnchorTarget(referenceSlug, {hash, pathname, search}),
  )

  const syncAnchor = React.useCallback(() => {
    const target = targetRef.current
    if (!target) {
      return
    }

    const applyAnchor = () => {
      if (`${globalThis.location.pathname}${globalThis.location.search}` !== target.path) {
        return
      }

      if (globalThis.location.hash !== target.hash) {
        globalThis.history.replaceState(globalThis.history.state, "", target.href)
      }

      document
        .querySelector<HTMLElement>(`#${CSS.escape(target.elementId)}`)
        ?.scrollIntoView({block: "start", inline: "nearest"})
    }

    applyAnchor()
    globalThis.requestAnimationFrame(applyAnchor)
  }, [])

  React.useEffect(() => {
    targetRef.current = apiReferenceAnchorTarget(referenceSlug, {hash, pathname, search})
    syncAnchor()
  }, [hash, pathname, referenceSlug, search, syncAnchor])

  return syncAnchor
}

function apiReferenceAnchorTarget(
  referenceSlug: string,
  location: ApiReferenceAnchorLocation,
): ApiReferenceAnchorTarget | undefined {
  return location.hash
    ? {
        path: `${location.pathname}${location.search}`,
        hash: location.hash,
        href: `${location.pathname}${location.search}${location.hash}`,
        elementId: `${referenceSlug}/${decodeHashPath(location.hash.slice(1))}`,
      }
    : undefined
}

function decodeHashPath(hashPath: string): string {
  try {
    return decodeURIComponent(hashPath)
  } catch {
    return hashPath
  }
}

function createToncenterApiFetch(
  apiBaseUrl: string,
  apiKey: string | undefined,
): typeof fetch | undefined {
  const toncenterApiKey = apiKey?.trim()
  if (!toncenterApiKey) {
    return undefined
  }

  const baseUrl = new URL(apiBaseUrl, globalThis.location.origin)
  return (input, init) => {
    const url = requestUrl(input)
    if (!url || !isUrlWithinBase(url, baseUrl)) {
      return fetch(input, init)
    }

    const headers = new Headers(
      init?.headers ?? (input instanceof Request ? input.headers : undefined),
    )
    headers.set("X-API-Key", toncenterApiKey)
    const requestInit = {...init, headers}
    return input instanceof Request
      ? fetch(new Request(input, requestInit))
      : fetch(input, requestInit)
  }
}

function requestUrl(input: string | URL | Request): URL | undefined {
  try {
    return new URL(input instanceof Request ? input.url : input, globalThis.location.origin)
  } catch {
    return undefined
  }
}

function isUrlWithinBase(url: URL, baseUrl: URL): boolean {
  const basePath = baseUrl.pathname.replace(/\/$/, "")
  return (
    url.origin === baseUrl.origin &&
    (url.pathname === basePath || url.pathname.startsWith(`${basePath}/`))
  )
}

function apiOrigin(apiBaseUrl: string): string {
  const normalizedBaseUrl = apiBaseUrl.trim()
  if (normalizedBaseUrl.length === 0) {
    return globalThis.location.origin
  }

  return new URL(normalizedBaseUrl, globalThis.location.origin).origin
}
