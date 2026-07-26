import {Navigate, Route, Routes, useLocation} from "react-router"
import {Check, KeyRound, ShieldCheck} from "lucide-react"
import {Dialog, Input} from "@acton/ui"
import {Suspense, lazy, useEffect, useLayoutEffect, useRef, useState} from "react"
import type {FC, ReactNode} from "react"

import dashboardStyles from "./dashboard/DashboardPage.module.css"
import {useExplorerPageTitle} from "./explorer/components/ExplorerDocumentTitle"
import {AccountPage} from "./explorer/pages/AccountPage"
import {AbiCatalogPage, AbiDetailsPage} from "./explorer/pages/AbiCatalogPage"
import {BlockDetailsPage, BlocksPage} from "./explorer/pages/BlocksPage"
import {CellInspectorPage} from "./explorer/pages/CellInspectorPage"
import {EmulatePage} from "./explorer/pages/EmulatePage"
import {ExplorerIndexPage} from "./explorer/pages/ExplorerIndexPage"
import {FavoriteAccountsPage} from "./explorer/pages/FavoriteAccountsPage"
import {SourceCatalogPage} from "./explorer/pages/SourceCatalogPage"
import {TransactionPage} from "./explorer/pages/TransactionPage"
import {AddressBookProvider} from "./explorer/hooks/useAddressBook"
import {MetadataRegistryProvider} from "./explorer/metadata/MetadataRegistryProvider"
import {FaucetPage} from "./dashboard/pages/FaucetPage"
import {HomePage} from "./dashboard/pages/HomePage"
import {NftsPage} from "./dashboard/pages/NftsPage"
import {TokensPage} from "./dashboard/pages/TokensPage"
import {WalletsPage} from "./dashboard/pages/WalletsPage"
import {useLocalnetRuntime} from "./LocalnetRuntimeProvider"
import {localnetPath} from "./routes"
import {WalletRuntimeProvider} from "./wallet/WalletRuntimeProvider"
import "@acton/ui/styles/tokens.css"
import "./index.css"
import styles from "./LocalnetWorkspace.module.css"

const ApiReferencePage = lazy(async () => {
  const module = await import("./dashboard/pages/ApiReferencePage")
  return {default: module.ApiReferencePage}
})
const ApiCallsPage = lazy(async () => {
  const module = await import("./dashboard/pages/ApiCallsPage")
  return {default: module.ApiCallsPage}
})

const LOCALNET_PAGE_TITLES: Readonly<Record<string, string>> = {
  "/dashboard": "Dashboard",
  "/faucet": "Faucet",
  "/wallets": "Wallets",
  "/tokens": "Tokens",
  "/nfts": "NFTs",
  "/api-reference/v2": "API Reference v2",
  "/api-reference/v3": "API Reference v3",
  "/api-reference/control": "Control API Reference",
  "/api-calls": "API Calls",
}

const LOCALNET_PAGE_DESCRIPTIONS: Readonly<Record<string, string>> = {
  "/dashboard": "Localnet status, network activity and runtime controls",
  "/faucet": "Fund accounts in this virtual environment",
  "/wallets": "Manage wallets available in this virtual environment",
  "/tokens": "Inspect token contracts and balances",
  "/nfts": "Inspect NFT collections and items",
  "/api-reference/v2": "Explore the localnet v2 API",
  "/api-reference/v3": "Explore the localnet v3 API",
  "/api-reference/control": "Explore localnet management methods",
  "/api-calls": "Review requests made to this virtual environment",
}

interface LocalnetWorkspaceProps {
  readonly basePath: string
  readonly onShellChange: (state: LocalnetWorkspaceShellState) => void
}

export interface LocalnetWorkspaceShellState {
  readonly pageDescription: string
  readonly pageTitle: string
  readonly rpcUrl?: string
}

export const LocalnetWorkspace: FC<LocalnetWorkspaceProps> = ({basePath, onShellChange}) => {
  const runtime = useLocalnetRuntime()

  return (
    <MetadataRegistryProvider registry={runtime.metadataRegistry}>
      <AddressBookProvider>
        <WalletRuntimeProvider
          apiBaseUrl={runtime.rpcBaseUrl}
          client={runtime.client}
          localnetApiToken={runtime.localnetApiToken}
        >
          <AppContent basePath={basePath} onShellChange={onShellChange} />
        </WalletRuntimeProvider>
      </AddressBookProvider>
    </MetadataRegistryProvider>
  )
}

interface AppContentProps {
  readonly basePath: string
  readonly onShellChange: (state: LocalnetWorkspaceShellState) => void
}

const AppContent: FC<AppContentProps> = ({basePath, onShellChange}) => {
  const runtime = useLocalnetRuntime()
  const client = runtime.client
  const {pathname} = useLocation()
  const explorerPageTitle = useExplorerPageTitle()
  const localPathname = pathname.slice(basePath.length) || "/"
  const pageTitle =
    explorerPageTitle ?? LOCALNET_PAGE_TITLES[localPathname] ?? "Virtual Environment"
  const pageDescription =
    LOCALNET_PAGE_DESCRIPTIONS[localPathname] ??
    "Inspect blocks, accounts, transactions and contract activity"
  const path = (value: string) => localnetPath(basePath, value)

  useLayoutEffect(() => {
    onShellChange({pageDescription, pageTitle, rpcUrl: runtime.rpcBaseUrl})
  }, [onShellChange, pageDescription, pageTitle, runtime.rpcBaseUrl])

  return (
    <>
      <LocalnetDocumentTitle
        environmentName={runtime.environment?.name ?? "Virtual Environment"}
        pageTitle={pageTitle}
      />
      <div className={styles.app}>
        <main className={styles.main}>
          <Routes>
            <Route path={basePath} element={<Navigate to={path("/dashboard")} replace />} />
            <Route
              path={path("/dashboard")}
              element={
                <DashboardPage>
                  <RouteSuspense>
                    <HomePage client={client} />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path={path("/faucet")}
              element={
                <DashboardPage>
                  <FaucetPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/blocks")}
              element={<Navigate to={path("/explorer/blocks")} replace />}
            />
            <Route
              path={path("/explorer/blocks")}
              element={
                <DashboardPage embedded>
                  <BlocksPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/block/last")}
              element={
                <DashboardPage embedded>
                  <BlockDetailsPage client={client} latest />
                </DashboardPage>
              }
            />
            <Route
              path={path("/block/:workchain/:shard/:seqno")}
              element={
                <DashboardPage embedded>
                  <BlockDetailsPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/wallets")}
              element={
                <DashboardPage>
                  <WalletsPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/tokens")}
              element={
                <DashboardPage>
                  <TokensPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/nfts")}
              element={
                <DashboardPage>
                  <NftsPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/api-reference")}
              element={<Navigate to={path("/api-reference/v2")} replace />}
            />
            <Route
              path={path("/api-reference/v2")}
              element={
                <DashboardPage embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={runtime.apiV2BaseUrl}
                      localnetApiToken={runtime.localnetApiToken}
                      onUnauthorized={runtime.requireAuthToken}
                      version="v2"
                    />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path={path("/api-reference/v3")}
              element={
                <DashboardPage embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={runtime.apiV3BaseUrl}
                      localnetApiToken={runtime.localnetApiToken}
                      onUnauthorized={runtime.requireAuthToken}
                      version="v3"
                    />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path={path("/api-reference/control")}
              element={
                <DashboardPage embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={runtime.rpcBaseUrl}
                      localnetApiToken={runtime.localnetApiToken}
                      onUnauthorized={runtime.requireAuthToken}
                      version="control"
                    />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path={path("/dashboard/faucet")}
              element={<Navigate to={path("/faucet")} replace />}
            />
            <Route
              path={path("/dashboard/wallets")}
              element={<Navigate to={path("/wallets")} replace />}
            />
            <Route
              path={path("/dashboard/tokens")}
              element={<Navigate to={path("/tokens")} replace />}
            />
            <Route
              path={path("/dashboard/nfts")}
              element={<Navigate to={path("/nfts")} replace />}
            />
            <Route
              path={path("/dashboard/json-rpc-calls")}
              element={<Navigate to={path("/api-calls")} replace />}
            />
            <Route
              path={path("/dashboard/api-calls")}
              element={<Navigate to={path("/api-calls")} replace />}
            />
            <Route
              path={path("/json-rpc-calls")}
              element={<Navigate to={path("/api-calls")} replace />}
            />
            <Route
              path={path("/api-calls")}
              element={
                <DashboardPage>
                  <RouteSuspense>
                    <ApiCallsPage client={client} />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer")}
              element={
                <DashboardPage embedded>
                  <ExplorerIndexPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/abi")}
              element={
                <DashboardPage embedded>
                  <AbiCatalogPage />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/abi/:slug")}
              element={
                <DashboardPage embedded>
                  <AbiDetailsPage />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/sources")}
              element={
                <DashboardPage embedded>
                  <SourceCatalogPage />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/cell")}
              element={
                <DashboardPage embedded>
                  <CellInspectorPage />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/emulate")}
              element={
                <DashboardPage embedded>
                  <EmulatePage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/favorites")}
              element={
                <DashboardPage embedded>
                  <FavoriteAccountsPage client={client} />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/address/:address")}
              element={
                <DashboardPage embedded>
                  <AccountPage client={client} enableJettonMint />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/tx/:hash/trace")}
              element={
                <DashboardPage embedded>
                  <TransactionPage client={client} openRetraceOnLoad />
                </DashboardPage>
              }
            />
            <Route
              path={path("/explorer/tx/:hash")}
              element={
                <DashboardPage embedded>
                  <TransactionPage client={client} />
                </DashboardPage>
              }
            />
            <Route path={`${basePath}/*`} element={<Navigate to={path("/dashboard")} replace />} />
          </Routes>
        </main>
      </div>

      {runtime.isAuthOverlayOpen && (
        <LocalnetAuthOverlay
          localnetApiToken={runtime.localnetApiToken}
          onClear={runtime.clearAuthToken}
          onClose={runtime.closeAuthOverlay}
          onSave={runtime.saveAuthToken}
          required={runtime.isAuthOverlayRequired}
        />
      )}
    </>
  )
}

interface DashboardPageProps {
  readonly children?: ReactNode
  readonly embedded?: boolean
}

const DashboardPage: FC<DashboardPageProps> = ({children, embedded = false}) => (
  <div
    className={`${dashboardStyles.content} ${styles.pageContent} ${
      embedded ? dashboardStyles.contentEmbedded : ""
    }`}
  >
    {embedded ? <div className={dashboardStyles.embeddedPage}>{children}</div> : children}
  </div>
)

const LocalnetDocumentTitle: FC<{
  readonly environmentName: string
  readonly pageTitle: string
}> = ({environmentName, pageTitle}) => <title>{`${pageTitle} · ${environmentName}`}</title>

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

  const title = required ? "Localnet API token required" : "Localnet API token"
  const description =
    required && localnetApiToken
      ? "The saved token was rejected by the localnet API. Paste the current token printed by the running localnet process."
      : "Paste the localnet API token to use protected routes from this browser. The token will be saved locally."

  return (
    <Dialog
      open
      title={title}
      description={description}
      className={styles.authDialog}
      leadingIcon={
        <span className={styles.authIcon} aria-hidden="true">
          <ShieldCheck size={21} />
        </span>
      }
      maxWidth={440}
      dismissible={canDismiss}
      closeLabel="Close localnet API token dialog"
      onOpenChange={open => {
        if (!open) onClose()
      }}
    >
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
        <Input
          ref={inputRef}
          id="localnet-api-token"
          className={styles.authInput}
          type="password"
          label="API token"
          leadingIcon={<KeyRound size={17} />}
          value={draftToken}
          placeholder="Paste token"
          onChange={event => setDraftToken(event.target.value)}
        />

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
    </Dialog>
  )
}
