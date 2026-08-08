import {Navigate, Route, Routes, useLocation} from "react-router"
import {Check, KeyRound, ShieldCheck} from "lucide-react"
import {Dialog, Input} from "@acton/ui"
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

import {supports} from "../environmentCapabilities"
import type {EnvironmentCapability, StudioEnvironment} from "../studioApi"
import dashboardStyles from "./dashboard/DashboardPage.module.css"
import {AccountPage} from "@acton/explorer-core/pages/AccountPage"
import {BlockDetailsPage, BlocksPage} from "@acton/explorer-core/pages/BlocksPage"
import {CellInspectorPage} from "@acton/explorer-core/pages/CellInspectorPage"
import {EmulatePage} from "@acton/explorer-core/pages/EmulatePage"
import {ExplorerIndexPage} from "@acton/explorer-core/pages/ExplorerIndexPage"
import {FavoriteAccountsPage} from "@acton/explorer-core/pages/FavoriteAccountsPage"
import {SuspendedAddressesPage} from "@acton/explorer-core/pages/SuspendedAddressesPage"
import {TransactionPage} from "@acton/explorer-core/pages/TransactionPage"
import {AddressBookProvider} from "@acton/explorer-core/hooks/useAddressBook"
import {MetadataRegistryProvider} from "@acton/explorer-core/metadata/MetadataRegistryProvider"
import {FaucetPage} from "./dashboard/pages/FaucetPage"
import {HomePage} from "./dashboard/pages/HomePage"
import {AbiCatalogPage, AbiDetailsPage} from "./dashboard/pages/AbiCatalogPage"
import {ApiCallsPage} from "./dashboard/pages/ApiCallsPage"
import {IntegratePage} from "./dashboard/pages/IntegratePage"
import {ContractPage} from "./dashboard/pages/ContractPage"
import {ContractsPage} from "./dashboard/pages/ContractsPage"
import {NftsPage} from "./dashboard/pages/NftsPage"
import {SettingsPage} from "./dashboard/pages/SettingsPage"
import {SnapshotsPage} from "./dashboard/pages/SnapshotsPage"
import {SourceCatalogPage} from "./dashboard/pages/SourceCatalogPage"
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
const LOCALNET_PAGE_TITLES: Readonly<Record<string, string>> = {
  "/dashboard": "Dashboard",
  "/faucet": "Faucet",
  "/wallets": "Wallets",
  "/simulator": "Simulator",
  "/cell-inspector": "Cell Inspector",
  "/contracts": "Contracts",
  "/contracts/sources": "Sources",
  "/contracts/abi": "ABI",
  "/explorer/tokens": "Tokens",
  "/explorer/nfts": "NFTs",
  "/explorer/suspended": "Suspended addresses",
  "/settings": "Settings",
  "/snapshots": "Snapshots",
  "/integrate": "Integrate",
  "/api-reference/v2": "API Reference v2",
  "/api-reference/v3": "API Reference v3",
  "/api-reference/admin": "Admin API Reference",
  "/api-reference/config": "Config API Reference",
  "/api-reference/control": "Control API Reference",
  "/api-calls": "API Calls",
}

const LOCALNET_PAGE_DESCRIPTIONS: Readonly<Record<string, string>> = {
  "/dashboard": "Network status and recent activity",
  "/faucet": "Fund accounts in this environment",
  "/wallets": "Project wallets available on this network, ready for TON Connect",
  "/simulator": "Build and replay messages against this network",
  "/cell-inspector": "Decode cells and inspect serialized TON data",
  "/contracts": "Track deployed contracts and match them with source artifacts",
  "/contracts/sources": "Manage source artifacts available to contracts on this network",
  "/contracts/abi": "Manage ABI used to decode contract state and messages",
  "/explorer/tokens": "Jettons detected on this network",
  "/explorer/nfts": "NFT items indexed from this network",
  "/explorer/suspended": "Addresses restricted by the network configuration",
  "/settings": "Manage environment identity, network behavior and mining",
  "/snapshots": "Create and restore persistent network snapshots",
  "/integrate": "Connect Acton projects, applications and TON-compatible tools to this network",
  "/api-reference/v2": "Explore the v2 API",
  "/api-reference/v3": "Explore the v3 API",
  "/api-reference/admin": "Inspect and manage Full localnet services",
  "/api-reference/config": "Read Full localnet configuration and connection endpoints",
  "/api-reference/control": "Explore network management methods",
  "/api-calls": "Review requests made to this environment",
}

interface LocalnetWorkspaceProps {
  readonly basePath: string
  readonly onEnvironmentChange: (environment: StudioEnvironment) => void
  readonly onEnvironmentDelete: (environmentId: string) => void
  readonly onShellChange: (state: LocalnetWorkspaceShellState) => void
}

export interface LocalnetWorkspaceShellState {
  readonly pageDescription: string
  readonly pageTitle: string
  readonly primaryAction?: LocalnetWorkspaceShellAction
  readonly rpcUrl?: string
}

export interface LocalnetWorkspaceShellAction {
  readonly icon: "archive" | "plus"
  readonly label: string
  readonly onClick: () => void
}

export const LocalnetWorkspace: FC<LocalnetWorkspaceProps> = ({
  basePath,
  onEnvironmentChange,
  onEnvironmentDelete,
  onShellChange,
}) => {
  const runtime = useLocalnetRuntime()
  const environment = runtime.environment
  const content = (
    <AppContent
      basePath={basePath}
      onEnvironmentChange={onEnvironmentChange}
      onEnvironmentDelete={onEnvironmentDelete}
      onShellChange={onShellChange}
    />
  )

  return (
    <MetadataRegistryProvider registry={runtime.metadataRegistry}>
      <AddressBookProvider>
        {environment?.status === "running" && supports(environment, "wallets") ? (
          <WalletRuntimeProvider
            key={environment.id}
            apiBaseUrl={runtime.rpcBaseUrl}
            environmentId={environment.id}
            environmentKind={environment.config.kind}
            localnetApiToken={runtime.localnetApiToken}
            networkLabel={environment.network.label}
            chainId={environment.network.chainId}
          >
            {content}
          </WalletRuntimeProvider>
        ) : (
          content
        )}
      </AddressBookProvider>
    </MetadataRegistryProvider>
  )
}

interface AppContentProps {
  readonly basePath: string
  readonly onEnvironmentChange: (environment: StudioEnvironment) => void
  readonly onEnvironmentDelete: (environmentId: string) => void
  readonly onShellChange: (state: LocalnetWorkspaceShellState) => void
}

const AppContent: FC<AppContentProps> = ({
  basePath,
  onEnvironmentChange,
  onEnvironmentDelete,
  onShellChange,
}) => {
  const runtime = useLocalnetRuntime()
  const client = runtime.client
  const {pathname} = useLocation()
  const [isAddContractOpen, setIsAddContractOpen] = useState(false)
  const [isCreateSnapshotOpen, setIsCreateSnapshotOpen] = useState(false)
  const localPathname = pathname.slice(basePath.length) || "/"
  const allowsOverflow = localPathname === "/faucet"
  const isExplorerPage = localPathname === "/explorer" || localPathname.startsWith("/explorer/")
  const isAbiDetailsPage = /^\/contracts\/abi\/[^/]+$/.test(localPathname)
  const pageTitle = isExplorerPage
    ? "Explorer"
    : isAbiDetailsPage
      ? "ABI"
      : (LOCALNET_PAGE_TITLES[localPathname] ??
        contractDetailsPageTitle(localPathname) ??
        "Virtual Environment")
  const pageDescription =
    LOCALNET_PAGE_DESCRIPTIONS[localPathname] ??
    contractDetailsPageDescription(localPathname) ??
    "Inspect blocks, accounts, transactions and contract activity"
  const path = (value: string) => localnetPath(basePath, value)
  const fallback = <Navigate to={path("/dashboard")} replace />
  const withCapability = (capability: EnvironmentCapability, page: ReactNode) =>
    supports(runtime.environment, capability) ? page : fallback
  const isFullLocalnet = runtime.environment?.config.kind === "fullTonNetwork"
  const openAddContract = useCallback(() => setIsAddContractOpen(true), [])
  const openCreateSnapshot = useCallback(() => setIsCreateSnapshotOpen(true), [])
  const primaryAction = useMemo<LocalnetWorkspaceShellAction | undefined>(() => {
    if (localPathname === "/contracts" && supports(runtime.environment, "contracts")) {
      return {icon: "plus", label: "Add contract", onClick: openAddContract}
    }
    if (localPathname === "/snapshots" && supports(runtime.environment, "snapshots")) {
      return {icon: "archive", label: "Create snapshot", onClick: openCreateSnapshot}
    }
    return undefined
  }, [localPathname, openAddContract, openCreateSnapshot, runtime.environment])
  const primaryEndpoint =
    runtime.environment?.endpoints.apiV3 ??
    runtime.environment?.endpoints.apiV2 ??
    runtime.environment?.endpoints.control

  useLayoutEffect(() => {
    onShellChange({
      pageDescription,
      pageTitle,
      primaryAction,
      rpcUrl: primaryEndpoint ? absoluteUrl(primaryEndpoint) : undefined,
    })
  }, [onShellChange, pageDescription, pageTitle, primaryAction, primaryEndpoint])

  useEffect(() => {
    if (localPathname !== "/contracts") setIsAddContractOpen(false)
    if (localPathname !== "/snapshots") setIsCreateSnapshotOpen(false)
  }, [localPathname])

  return (
    <>
      <LocalnetDocumentTitle
        environmentName={runtime.environment?.name ?? "Virtual Environment"}
        pageTitle={pageTitle}
      />
      <div className={`${styles.app} ${allowsOverflow ? styles.allowsOverflow : ""}`}>
        <main className={`${styles.main} ${allowsOverflow ? styles.allowsOverflow : ""}`}>
          <Routes>
            <Route path={basePath} element={<Navigate to={path("/dashboard")} replace />} />
            <Route
              path={path("/dashboard")}
              element={
                <DashboardPage embedded>
                  <RouteSuspense>
                    <HomePage client={client} />
                  </RouteSuspense>
                </DashboardPage>
              }
            />
            <Route
              path={path("/faucet")}
              element={
                runtime.gramFaucetEnabled || runtime.jettonFaucetEnabled ? (
                  <DashboardPage>
                    <FaucetPage
                      client={client}
                      gramFaucetEnabled={runtime.gramFaucetEnabled}
                      jettonFaucetEnabled={runtime.jettonFaucetEnabled}
                      projectWalletsEnabled={supports(runtime.environment, "wallets")}
                    />
                  </DashboardPage>
                ) : (
                  fallback
                )
              }
            />
            <Route
              path={path("/blocks")}
              element={<Navigate to={path("/explorer/blocks")} replace />}
            />
            <Route
              path={path("/explorer/blocks")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <BlocksPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/block/last")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <BlockDetailsPage client={client} latest />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/block/:workchain/:shard/:seqno")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <BlockDetailsPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/wallets")}
              element={withCapability(
                "wallets",
                <DashboardPage>
                  <WalletsPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/explorer/tokens")}
              element={withCapability(
                "explorer",
                <DashboardPage>
                  <TokensPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/explorer/nfts")}
              element={withCapability(
                "explorer",
                <DashboardPage>
                  <NftsPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/settings")}
              element={
                runtime.environment?.lifecycle === "managed" ? (
                  <DashboardPage>
                    <SettingsPage
                      client={client}
                      onEnvironmentChange={onEnvironmentChange}
                      onEnvironmentDelete={onEnvironmentDelete}
                    />
                  </DashboardPage>
                ) : (
                  fallback
                )
              }
            />
            <Route
              path={path("/snapshots")}
              element={withCapability(
                "snapshots",
                <DashboardPage>
                  {runtime.environment ? (
                    <SnapshotsPage
                      createOpen={isCreateSnapshotOpen}
                      environment={runtime.environment}
                      onCreateOpenChange={setIsCreateSnapshotOpen}
                    />
                  ) : (
                    fallback
                  )}
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/integrate")}
              element={withCapability(
                "integration",
                <DashboardPage>
                  <IntegratePage />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/contracts")}
              element={withCapability(
                "contracts",
                <DashboardPage>
                  <ContractsPage
                    addOpen={isAddContractOpen}
                    client={client}
                    onAddOpenChange={setIsAddContractOpen}
                  />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/contracts/abi")}
              element={withCapability(
                "contracts",
                <DashboardPage>
                  <AbiCatalogPage />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/contracts/abi/:slug")}
              element={withCapability(
                "contracts",
                <DashboardPage>
                  <AbiDetailsPage />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/contracts/sources")}
              element={withCapability(
                "contracts",
                <DashboardPage>
                  <SourceCatalogPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/contracts/:address")}
              element={withCapability(
                "contracts",
                <DashboardPage>
                  <ContractPage client={client} section="source" />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/contracts/:address/abi")}
              element={withCapability(
                "contracts",
                <DashboardPage>
                  <ContractPage client={client} section="abi" />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/contracts/:address/raw-abi")}
              element={withCapability(
                "contracts",
                <DashboardPage>
                  <ContractPage client={client} section="raw-abi" />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/api-reference")}
              element={
                <Navigate
                  to={path(
                    supports(runtime.environment, "apiV2")
                      ? "/api-reference/v2"
                      : supports(runtime.environment, "apiV3")
                        ? "/api-reference/v3"
                        : "/dashboard",
                  )}
                  replace
                />
              }
            />
            <Route
              path={path("/api-reference/v2")}
              element={withCapability(
                "apiV2",
                <DashboardPage embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={runtime.apiV2BaseUrl}
                      localnetApiToken={runtime.localnetApiToken}
                      onUnauthorized={runtime.requireAuthToken}
                      version="v2"
                    />
                  </RouteSuspense>
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/api-reference/v3")}
              element={withCapability(
                "apiV3",
                <DashboardPage embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={runtime.apiV3BaseUrl}
                      localnetApiToken={runtime.localnetApiToken}
                      onUnauthorized={runtime.requireAuthToken}
                      version="v3"
                    />
                  </RouteSuspense>
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/api-reference/admin")}
              element={
                isFullLocalnet
                  ? withCapability(
                      "controlApi",
                      <DashboardPage embedded>
                        <RouteSuspense>
                          <ApiReferencePage
                            apiBaseUrl={
                              runtime.environment?.endpoints.control ?? runtime.rpcBaseUrl
                            }
                            localnetApiToken={runtime.localnetApiToken}
                            onUnauthorized={runtime.requireAuthToken}
                            version="admin"
                          />
                        </RouteSuspense>
                      </DashboardPage>,
                    )
                  : fallback
              }
            />
            <Route
              path={path("/api-reference/config")}
              element={withCapability(
                "configApi",
                <DashboardPage embedded>
                  <RouteSuspense>
                    <ApiReferencePage
                      apiBaseUrl={runtime.environment?.endpoints.config ?? runtime.rpcBaseUrl}
                      localnetApiToken={runtime.localnetApiToken}
                      onUnauthorized={runtime.requireAuthToken}
                      version="config"
                    />
                  </RouteSuspense>
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/api-reference/control")}
              element={
                isFullLocalnet
                  ? fallback
                  : withCapability(
                      "controlApi",
                      <DashboardPage embedded>
                        <RouteSuspense>
                          <ApiReferencePage
                            apiBaseUrl={
                              runtime.environment?.endpoints.control ?? runtime.rpcBaseUrl
                            }
                            localnetApiToken={runtime.localnetApiToken}
                            onUnauthorized={runtime.requireAuthToken}
                            version="control"
                          />
                        </RouteSuspense>
                      </DashboardPage>,
                    )
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
              element={withCapability(
                "apiCalls",
                <DashboardPage>
                  {runtime.environment ? (
                    <ApiCallsPage environmentId={runtime.environment.id} />
                  ) : (
                    fallback
                  )}
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/explorer")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <ExplorerIndexPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/cell-inspector")}
              element={withCapability(
                "simulator",
                <DashboardPage embedded>
                  <CellInspectorPage />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/simulator")}
              element={withCapability(
                "simulator",
                <DashboardPage embedded>
                  <EmulatePage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/explorer/favorites")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <FavoriteAccountsPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/explorer/suspended")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <SuspendedAddressesPage client={client} />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/explorer/address/:address")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <AccountPage
                    client={client}
                    enableJettonMint={runtime.jettonFaucetEnabled}
                    jettonMintPath={path("/faucet")}
                    showActonscanLink
                    enableTransactionStreaming={
                      runtime.environment?.status === "running" &&
                      supports(runtime.environment, "controlApi")
                    }
                  />
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/explorer/tx/:hash/trace")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <div className={styles.transactionDebugPage}>
                    <TransactionPage client={client} openRetraceOnLoad />
                  </div>
                </DashboardPage>,
              )}
            />
            <Route
              path={path("/explorer/tx/:hash")}
              element={withCapability(
                "explorer",
                <DashboardPage embedded>
                  <div className={styles.transactionDebugPage}>
                    <TransactionPage client={client} />
                  </div>
                </DashboardPage>,
              )}
            />
            <Route path={`${basePath}/*`} element={<Navigate to={path("/dashboard")} replace />} />
          </Routes>
        </main>
      </div>

      {runtime.isAuthOverlayOpen && supports(runtime.environment, "controlApi") && (
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

function contractDetailsPageTitle(localPathname: string): string | undefined {
  if (/^\/contracts\/abi\/[^/]+$/.test(localPathname)) {
    return "ABI"
  }

  return /^\/contracts\/[^/]+(?:\/(?:abi|raw-abi))?$/.test(localPathname) ? "Contract" : undefined
}

function contractDetailsPageDescription(localPathname: string): string | undefined {
  if (/^\/contracts\/abi\/[^/]+$/.test(localPathname)) {
    return "Inspect contract declarations, messages, getters and errors"
  }

  return /^\/contracts\/[^/]+(?:\/(?:abi|raw-abi))?$/.test(localPathname)
    ? "Inspect deployed code, ABI and project artifacts"
    : undefined
}

function absoluteUrl(value: string): string {
  try {
    return new URL(value, globalThis.location.origin).href
  } catch {
    return value
  }
}

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
