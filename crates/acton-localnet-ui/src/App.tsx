import {BrowserRouter, Navigate, Route, Routes} from "react-router-dom"
import {Check, KeyRound, ShieldCheck, X} from "lucide-react"
import {ToastProvider} from "@acton/shared-ui"
import type {ThemeMode} from "@acton/shared-ui"
import {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import type {FC, ReactNode} from "react"

import {TonClient} from "./explorer/api/client"
import {getBundledCompilerAbis} from "./explorer/api/compilerAbiCatalog"
import {AccountPage} from "./explorer/pages/AccountPage"
import {AbiCatalogPage, AbiDetailsPage} from "./explorer/pages/AbiCatalogPage"
import {BlockDetailsPage, BlocksPage} from "./explorer/pages/BlocksPage"
import {ExplorerIndexPage} from "./explorer/pages/ExplorerIndexPage"
import {SourceCatalogPage} from "./explorer/pages/SourceCatalogPage"
import {TransactionPage} from "./explorer/pages/TransactionPage"
import {NetworkInfoProvider} from "./explorer/hooks/NetworkInfoProvider"
import {AddressBookProvider} from "./explorer/hooks/useAddressBook"
import {BundledAbiRegistry} from "./explorer/metadata/bundledAbiRegistry"
import {CompositeMetadataRegistry} from "./explorer/metadata/compositeRegistry"
import {LocalnetMetadataRegistry} from "./explorer/metadata/localnetRegistry"
import {MetadataRegistryProvider} from "./explorer/metadata/MetadataRegistryProvider"
import {VerifierMetadataRegistry} from "./explorer/metadata/verifierRegistry"
import {DashboardPage} from "./dashboard/DashboardPage"
import {FaucetPage} from "./dashboard/pages/FaucetPage"
import {HomePage} from "./dashboard/pages/HomePage"
import {NftsPage} from "./dashboard/pages/NftsPage"
import {TokensPage} from "./dashboard/pages/TokensPage"
import {WalletsPage} from "./dashboard/pages/WalletsPage"
import {WalletRuntimeProvider} from "./wallet/WalletRuntimeProvider"
import "@acton/shared-ui/styles/tokens.css"
import "./index.css"
import styles from "./App.module.css"

const HOST = (import.meta.env.VITE_LOCALNET_HOST || "").replace(/\/$/, "")
const LOCALNET_API_TOKEN_STORAGE_KEY = "localnetApiToken"
const ENV_LOCALNET_API_TOKEN = import.meta.env.VITE_LOCALNET_API_TOKEN?.trim() || undefined
const TONCENTER_API_KEY = import.meta.env.VITE_LOCALNET_TONCENTER_API_KEY?.trim() || undefined
const TONCENTER_API_V2_URL =
  import.meta.env.VITE_LOCALNET_TONCENTER_API_V2_URL?.trim().replace(/\/$/, "") || undefined
const TONCENTER_API_V3_URL =
  import.meta.env.VITE_LOCALNET_TONCENTER_API_V3_URL?.trim().replace(/\/$/, "") || undefined
const API_V2_BASE_URL = TONCENTER_API_V2_URL ?? `${HOST}/api/v2`
const API_V3_BASE_URL = TONCENTER_API_V3_URL ?? `${HOST}/api/v3`

const readInitialTheme = (): ThemeMode => {
  const storedTheme = localStorage.getItem("theme")
  if (storedTheme === "dark" || storedTheme === "light") {
    return storedTheme
  }

  return globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
}

const readInitialLocalnetApiToken = (): string | undefined => {
  return ENV_LOCALNET_API_TOKEN || localStorage.getItem(LOCALNET_API_TOKEN_STORAGE_KEY) || undefined
}

const ApiReferencePage = lazy(async () => {
  const module = await import("./dashboard/pages/ApiReferencePage")
  return {default: module.ApiReferencePage}
})
const ApiCallsPage = lazy(async () => {
  const module = await import("./dashboard/pages/ApiCallsPage")
  return {default: module.ApiCallsPage}
})
export const App: FC = () => {
  const [theme, setTheme] = useState<ThemeMode>(readInitialTheme)
  const [localnetApiToken, setLocalnetApiTokenState] = useState<string | undefined>(
    readInitialLocalnetApiToken,
  )
  const [isAuthOverlayOpen, setIsAuthOverlayOpen] = useState(false)
  const [isAuthOverlayRequired, setIsAuthOverlayRequired] = useState(false)

  const setLocalnetApiToken = useCallback((token: string | undefined) => {
    const nextToken = token?.trim() || undefined
    if (nextToken) {
      localStorage.setItem(LOCALNET_API_TOKEN_STORAGE_KEY, nextToken)
    } else {
      localStorage.removeItem(LOCALNET_API_TOKEN_STORAGE_KEY)
    }
    setLocalnetApiTokenState(nextToken)
  }, [])

  const openAuthOverlay = useCallback(() => {
    setIsAuthOverlayRequired(false)
    setIsAuthOverlayOpen(true)
  }, [])

  const closeAuthOverlay = useCallback(() => {
    setIsAuthOverlayRequired(false)
    setIsAuthOverlayOpen(false)
  }, [])

  const handleUnauthorized = useCallback(() => {
    setIsAuthOverlayRequired(true)
    setIsAuthOverlayOpen(true)
  }, [])

  const saveAuthToken = useCallback(
    (token: string) => {
      setLocalnetApiToken(token)
      setIsAuthOverlayRequired(false)
      setIsAuthOverlayOpen(false)
    },
    [setLocalnetApiToken],
  )

  const clearAuthToken = useCallback(() => {
    setLocalnetApiToken(undefined)
    if (!isAuthOverlayRequired) {
      setIsAuthOverlayOpen(false)
    }
  }, [isAuthOverlayRequired, setLocalnetApiToken])

  const client = useMemo(
    () =>
      new TonClient({
        v2BaseUrl: API_V2_BASE_URL,
        v3BaseUrl: API_V3_BASE_URL,
        addressNameBaseUrl: HOST,
        localnetApiToken,
        onUnauthorized: handleUnauthorized,
        toncenterApiKey: TONCENTER_API_KEY,
      }),
    [handleUnauthorized, localnetApiToken],
  )
  const metadataRegistry = useMemo(
    () =>
      new CompositeMetadataRegistry([
        new LocalnetMetadataRegistry(client),
        new BundledAbiRegistry(getBundledCompilerAbis),
        new VerifierMetadataRegistry(),
      ]),
    [client],
  )
  const explorerApi = useMemo(
    () => ({
      v2BaseUrl: API_V2_BASE_URL,
      v3BaseUrl: API_V3_BASE_URL,
      toncenterApiKey: TONCENTER_API_KEY,
    }),
    [],
  )
  useLayoutEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    document.body.classList.toggle("dark-mode", theme === "dark")
    document.body.classList.toggle("light-mode", theme !== "dark")
    localStorage.setItem("theme", theme)
  }, [theme])

  return (
    <BrowserRouter>
      <ToastProvider>
        <NetworkInfoProvider client={client} api={explorerApi}>
          <MetadataRegistryProvider registry={metadataRegistry}>
            <AddressBookProvider>
              <WalletRuntimeProvider
                client={client}
                host={HOST}
                localnetApiToken={localnetApiToken}
              >
                <AppContent
                  client={client}
                  isAuthOverlayOpen={isAuthOverlayOpen}
                  isAuthOverlayRequired={isAuthOverlayRequired}
                  localnetApiToken={localnetApiToken}
                  onClearAuthToken={clearAuthToken}
                  onCloseAuthOverlay={closeAuthOverlay}
                  onOpenAuthOverlay={openAuthOverlay}
                  onRequireAuthToken={handleUnauthorized}
                  onSaveAuthToken={saveAuthToken}
                  theme={theme}
                  setTheme={setTheme}
                />
              </WalletRuntimeProvider>
            </AddressBookProvider>
          </MetadataRegistryProvider>
        </NetworkInfoProvider>
      </ToastProvider>
    </BrowserRouter>
  )
}

interface AppContentProps {
  readonly client: TonClient
  readonly isAuthOverlayOpen: boolean
  readonly isAuthOverlayRequired: boolean
  readonly localnetApiToken?: string
  readonly onClearAuthToken: () => void
  readonly onCloseAuthOverlay: () => void
  readonly onOpenAuthOverlay: () => void
  readonly onRequireAuthToken: () => void
  readonly onSaveAuthToken: (token: string) => void
  readonly theme: ThemeMode
  readonly setTheme: (theme: ThemeMode) => void
}

const AppContent: FC<AppContentProps> = ({
  client,
  isAuthOverlayOpen,
  isAuthOverlayRequired,
  localnetApiToken,
  onClearAuthToken,
  onCloseAuthOverlay,
  onOpenAuthOverlay,
  onRequireAuthToken,
  onSaveAuthToken,
  theme,
  setTheme,
}) => {
  const dashboardProps = {
    client,
    localnetApiToken,
    onOpenAuthTokenOverlay: onOpenAuthOverlay,
    theme,
    setTheme,
  }

  return (
    <>
      <div className={styles.app}>
        <main className={styles.main}>
          <Routes>
            <Route path="/" element={<Navigate to="/dashboard" replace />} />
            <Route
              path="/dashboard"
              element={
                <DashboardPage {...dashboardProps}>
                  <RouteSuspense>
                    <HomePage client={client} />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/faucet"
              element={
                <DashboardPage {...dashboardProps}>
                  <FaucetPage client={client} />
                </DashboardPage>
              }
            />
            <Route path="/blocks" element={<Navigate to="/explorer/blocks" replace />} />
            <Route
              path="/explorer/blocks"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <BlocksPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path="/block/:workchain/:shard/:seqno"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <BlockDetailsPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path="/wallets"
              element={
                <DashboardPage {...dashboardProps}>
                  <WalletsPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path="/tokens"
              element={
                <DashboardPage {...dashboardProps}>
                  <TokensPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path="/nfts"
              element={
                <DashboardPage {...dashboardProps}>
                  <NftsPage client={client} />
                </DashboardPage>
              }
            />
            <Route path="/api-reference" element={<Navigate to="/api-reference/v2" replace />} />
            <Route
              path="/api-reference/v2"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={API_V2_BASE_URL}
                      localnetApiToken={TONCENTER_API_V2_URL ? undefined : localnetApiToken}
                      onUnauthorized={onRequireAuthToken}
                      theme={theme}
                      toncenterApiKey={TONCENTER_API_KEY}
                      version="v2"
                    />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/api-reference/v3"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={API_V3_BASE_URL}
                      localnetApiToken={TONCENTER_API_V3_URL ? undefined : localnetApiToken}
                      onUnauthorized={onRequireAuthToken}
                      theme={theme}
                      toncenterApiKey={TONCENTER_API_KEY}
                      version="v3"
                    />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/api-reference/control"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={HOST}
                      localnetApiToken={localnetApiToken}
                      onUnauthorized={onRequireAuthToken}
                      theme={theme}
                      version="control"
                    />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route path="/dashboard/faucet" element={<Navigate to="/faucet" replace />} />
            <Route path="/dashboard/wallets" element={<Navigate to="/wallets" replace />} />
            <Route path="/dashboard/tokens" element={<Navigate to="/tokens" replace />} />
            <Route path="/dashboard/nfts" element={<Navigate to="/nfts" replace />} />
            <Route
              path="/dashboard/json-rpc-calls"
              element={<Navigate to="/api-calls" replace />}
            />
            <Route path="/dashboard/api-calls" element={<Navigate to="/api-calls" replace />} />
            <Route path="/json-rpc-calls" element={<Navigate to="/api-calls" replace />} />
            <Route
              path="/api-calls"
              element={
                <DashboardPage {...dashboardProps}>
                  <RouteSuspense>
                    <ApiCallsPage client={client} />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/explorer"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <ExplorerIndexPage />
                </DashboardPage>
              }
            />
            <Route
              path="/explorer/abi"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <AbiCatalogPage />
                </DashboardPage>
              }
            />
            <Route
              path="/explorer/abi/:slug"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <AbiDetailsPage />
                </DashboardPage>
              }
            />
            <Route
              path="/explorer/sources"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <SourceCatalogPage />
                </DashboardPage>
              }
            />
            <Route
              path="/explorer/address/:address"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <AccountPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path="/explorer/tx/:hash/trace"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <TransactionPage client={client} openRetraceOnLoad />
                </DashboardPage>
              }
            />
            <Route
              path="/explorer/tx/:hash"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <TransactionPage client={client} />
                </DashboardPage>
              }
            />
            <Route path="*" element={<Navigate to="/dashboard" replace />} />
          </Routes>
        </main>
      </div>

      {isAuthOverlayOpen && (
        <LocalnetAuthOverlay
          localnetApiToken={localnetApiToken}
          onClear={onClearAuthToken}
          onClose={onCloseAuthOverlay}
          onSave={onSaveAuthToken}
          required={isAuthOverlayRequired}
        />
      )}
    </>
  )
}

const RouteSuspense: FC<{readonly children: ReactNode}> = ({children}) => (
  <Suspense fallback={<div className={styles.routeLoading}>Loading…</div>}>{children}</Suspense>
)

interface LocalnetAuthOverlayProps {
  readonly localnetApiToken?: string
  readonly onClear: () => void
  readonly onClose: () => void
  readonly onSave: (token: string) => void
  readonly required: boolean
}

const LocalnetAuthOverlay: FC<LocalnetAuthOverlayProps> = ({
  localnetApiToken,
  onClear,
  onClose,
  onSave,
  required,
}) => {
  const [draftToken, setDraftToken] = useState(localnetApiToken ?? "")
  const inputRef = useRef<HTMLInputElement>(null)
  const canDismiss = !required

  useEffect(() => {
    setDraftToken(localnetApiToken ?? "")
  }, [localnetApiToken])

  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  useEffect(() => {
    if (!canDismiss) {
      return
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose()
      }
    }

    globalThis.addEventListener("keydown", onKeyDown)
    return () => globalThis.removeEventListener("keydown", onKeyDown)
  }, [canDismiss, onClose])

  const title = required ? "Localnet API token required" : "Localnet API token"
  const description =
    required && localnetApiToken
      ? "The saved token was rejected by the localnet API. Paste the current token printed by the running localnet process."
      : "Paste the localnet API token to use protected routes from this browser. The token will be saved locally."

  return (
    <div className={styles.authOverlay}>
      <button
        type="button"
        className={styles.authBackdrop}
        aria-label="Close localnet API token dialog"
        disabled={!canDismiss}
        onClick={onClose}
      />
      <section
        className={styles.authPanel}
        role="dialog"
        aria-modal="true"
        aria-labelledby="localnet-auth-title"
        aria-describedby="localnet-auth-description"
      >
        <div className={styles.authHeader}>
          <span className={styles.authIcon} aria-hidden="true">
            <ShieldCheck size={21} />
          </span>
          <div className={styles.authTitleBlock}>
            <h2 id="localnet-auth-title" className={styles.authTitle}>
              {title}
            </h2>
            <p id="localnet-auth-description" className={styles.authDescription}>
              {description}
            </p>
          </div>
          {canDismiss && (
            <button
              type="button"
              className={styles.authCloseButton}
              onClick={onClose}
              aria-label="Close localnet API token dialog"
            >
              <X size={17} />
            </button>
          )}
        </div>

        <form
          className={styles.authForm}
          onSubmit={event => {
            event.preventDefault()
            const nextToken = draftToken.trim()
            if (nextToken) {
              onSave(nextToken)
            }
          }}
        >
          <label className={styles.authLabel} htmlFor="localnet-api-token">
            API token
          </label>
          <div className={styles.authInputFrame}>
            <KeyRound size={17} aria-hidden="true" />
            <input
              ref={inputRef}
              id="localnet-api-token"
              className={styles.authInput}
              type="password"
              value={draftToken}
              placeholder="Paste token"
              onChange={event => setDraftToken(event.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
          </div>

          <div className={styles.authActions}>
            <button
              type="submit"
              className={`${styles.authActionButton} ${styles.authPrimaryButton}`}
              disabled={!draftToken.trim()}
            >
              <Check size={16} />
              <span>Save token</span>
            </button>
            {localnetApiToken && (
              <button
                type="button"
                className={styles.authActionButton}
                onClick={() => {
                  setDraftToken("")
                  onClear()
                }}
              >
                Clear stored token
              </button>
            )}
          </div>
        </form>
      </section>
    </div>
  )
}
