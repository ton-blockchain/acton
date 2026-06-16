import * as React from "react"
import {useCallback, useEffect, useMemo, useState} from "react"
import {BrowserRouter, Navigate, Route, Routes} from "react-router-dom"
import {Check, KeyRound, ShieldCheck, X} from "lucide-react"
import {ToastProvider} from "@acton/shared-ui"
import type {ThemeMode} from "@acton/shared-ui"

import {TonClient} from "./explorer/api/client"
import {NetworkInfoProvider} from "./explorer/hooks/NetworkInfoProvider"
import {AddressBookProvider} from "./explorer/hooks/useAddressBook"
import {DashboardPage} from "./dashboard/DashboardPage"
import {WalletRuntimeProvider} from "./wallet/WalletRuntimeProvider"
import "@acton/shared-ui/styles/tokens.css"
import "./index.css"
import styles from "./App.module.css"

const HOST = (import.meta.env.VITE_LOCALNET_HOST || "").replace(/\/$/, "")
const LOCALNET_API_TOKEN_STORAGE_KEY = "localnetApiToken"
const ENV_LOCALNET_API_TOKEN = import.meta.env.VITE_LOCALNET_API_TOKEN?.trim() || undefined
const TONCENTER_API_KEY = import.meta.env.VITE_LOCALNET_TONCENTER_API_KEY?.trim() || undefined

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

const ApiReferencePage = React.lazy(async () => {
  const module = await import("./dashboard/pages/ApiReferencePage")
  return {default: module.ApiReferencePage}
})
const FaucetPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/FaucetPage")
  return {default: module.FaucetPage}
})
const HomePage = React.lazy(async () => {
  const module = await import("./dashboard/pages/HomePage")
  return {default: module.HomePage}
})
const ApiCallsPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/ApiCallsPage")
  return {default: module.ApiCallsPage}
})
const NftsPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/NftsPage")
  return {default: module.NftsPage}
})
const TokensPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/TokensPage")
  return {default: module.TokensPage}
})
const WalletsPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/WalletsPage")
  return {default: module.WalletsPage}
})
const AccountPage = React.lazy(async () => {
  const module = await import("./explorer/pages/AccountPage")
  return {default: module.AccountPage}
})
const ExplorerIndexPage = React.lazy(async () => {
  const module = await import("./explorer/pages/ExplorerIndexPage")
  return {default: module.ExplorerIndexPage}
})
const TransactionPage = React.lazy(async () => {
  const module = await import("./explorer/pages/TransactionPage")
  return {default: module.TransactionPage}
})

export const App: React.FC = () => {
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
        v2BaseUrl: `${HOST}/api/v2`,
        v3BaseUrl: `${HOST}/api/v3`,
        addressNameBaseUrl: HOST,
        localnetApiToken,
        onUnauthorized: handleUnauthorized,
        toncenterApiKey: TONCENTER_API_KEY,
      }),
    [handleUnauthorized, localnetApiToken],
  )

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    document.body.classList.toggle("dark-mode", theme === "dark")
    document.body.classList.toggle("light-mode", theme !== "dark")
    localStorage.setItem("theme", theme)
  }, [theme])

  return (
    <BrowserRouter>
      <ToastProvider>
        <NetworkInfoProvider client={client}>
          <AddressBookProvider client={client}>
            <WalletRuntimeProvider client={client} host={HOST}>
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

const AppContent: React.FC<AppContentProps> = ({
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
                  <RouteSuspense>
                    <FaucetPage client={client} />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/wallets"
              element={
                <DashboardPage {...dashboardProps}>
                  <RouteSuspense>
                    <WalletsPage />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/tokens"
              element={
                <DashboardPage {...dashboardProps}>
                  <RouteSuspense>
                    <TokensPage client={client} />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/nfts"
              element={
                <DashboardPage {...dashboardProps}>
                  <RouteSuspense>
                    <NftsPage client={client} />
                  </RouteSuspense>
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
                      apiBaseUrl={`${HOST}/api/v2`}
                      localnetApiToken={localnetApiToken}
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
                      apiBaseUrl={`${HOST}/api/v3`}
                      localnetApiToken={localnetApiToken}
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
                  <RouteSuspense>
                    <ExplorerIndexPage />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/explorer/address/:address"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <RouteSuspense>
                    <AccountPage client={client} />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path="/explorer/tx/:hash"
              element={
                <DashboardPage {...dashboardProps} embedded>
                  <RouteSuspense>
                    <TransactionPage client={client} />
                  </RouteSuspense>
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

const RouteSuspense: React.FC<{readonly children: React.ReactNode}> = ({children}) => (
  <React.Suspense fallback={<div className={styles.routeLoading}>Loading…</div>}>
    {children}
  </React.Suspense>
)

interface LocalnetAuthOverlayProps {
  readonly localnetApiToken?: string
  readonly onClear: () => void
  readonly onClose: () => void
  readonly onSave: (token: string) => void
  readonly required: boolean
}

const LocalnetAuthOverlay: React.FC<LocalnetAuthOverlayProps> = ({
  localnetApiToken,
  onClear,
  onClose,
  onSave,
  required,
}) => {
  const [draftToken, setDraftToken] = React.useState(localnetApiToken ?? "")
  const inputRef = React.useRef<HTMLInputElement>(null)
  const canDismiss = !required

  React.useEffect(() => {
    setDraftToken(localnetApiToken ?? "")
  }, [localnetApiToken])

  React.useEffect(() => {
    inputRef.current?.focus()
  }, [])

  React.useEffect(() => {
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
