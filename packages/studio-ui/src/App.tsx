import {Archive, Boxes, FlaskConical, Plus} from "lucide-react"
import {lazy, Suspense, useCallback, useEffect, useState} from "react"
import {useLocation, useNavigate, useSearchParams} from "react-router"
import {Button, CopyButton, ToastProvider, useToast} from "@acton/ui"

import {StudioShell} from "./components/StudioShell"
import {useStudioEnvironments, type StudioEnvironmentsState} from "./hooks/useStudioEnvironments"
import {useStudioTestRuns} from "./hooks/useStudioTestRuns"
import {EnvironmentNavigation} from "./localnet/dashboard/EnvironmentNavigation"
import {EnvironmentNavigationActions} from "./localnet/dashboard/EnvironmentNavigationActions"
import {DashboardSearch} from "./localnet/dashboard/DashboardSearch"
import type {LocalnetWorkspaceShellState} from "./localnet/LocalnetWorkspace"
import {LocalnetRuntimeProvider, useLocalnetRuntime} from "./localnet/LocalnetRuntimeProvider"
import {FeaturePage} from "./pages/FeaturePage"
import {OverviewPage} from "./pages/OverviewPage"
import {TestsPage} from "./pages/TestsPage"
import {VirtualEnvironmentsPage} from "./pages/VirtualEnvironmentsPage"
import {
  fetchStudioInfo,
  type StudioConnectionState,
  type StudioEnvironment,
  type StudioInfo,
} from "./studioApi"
import {studioFeaturePages, studioPages, type StudioPath} from "./studioPages"
import {
  readStudioRoute,
  studioEnvironmentPath,
  type StudioRoute,
  testRunStudioPath,
} from "./studioRoutes"

const configuredProjectName = import.meta.env.VITE_STUDIO_PROJECT_NAME?.trim() || undefined
const configuredProjectPath = import.meta.env.VITE_STUDIO_PROJECT_PATH?.trim() || undefined
const EnvironmentWorkspacePage = lazy(async () => {
  const module = await import("./pages/EnvironmentWorkspacePage")
  return {default: module.EnvironmentWorkspacePage}
})

function StudioApp() {
  const location = useLocation()
  const routerNavigate = useNavigate()
  const route = readStudioRoute(location.pathname)
  const environmentsState = useStudioEnvironments()
  const environmentRoute = route.kind === "environment" ? route : undefined
  const environment = environmentsState.environments.find(
    candidate =>
      candidate.id === environmentRoute?.environmentId &&
      candidate.lifecycle === (environmentRoute.section === "networks" ? "external" : "managed"),
  )

  const navigate = useCallback(
    (path: StudioPath) => {
      void routerNavigate(path)
      globalThis.scrollTo({top: 0})
    },
    [routerNavigate],
  )

  const openEnvironment = useCallback(
    (environment: StudioEnvironment) => {
      environmentsState.setEnvironment(environment)
      void routerNavigate(studioEnvironmentPath(environment))
      globalThis.scrollTo({top: 0})
    },
    [environmentsState.setEnvironment, routerNavigate],
  )

  const deleteEnvironment = useCallback(
    (environmentId: string) => {
      environmentsState.removeEnvironment(environmentId)
      void routerNavigate("/virtual-environments")
      globalThis.scrollTo({top: 0})
    },
    [environmentsState.removeEnvironment, routerNavigate],
  )

  const selectTestRun = useCallback(
    (runId: string | undefined, replace: boolean) => {
      void routerNavigate(runId ? testRunStudioPath(runId) : "/tests", {replace})
    },
    [routerNavigate],
  )

  return (
    <LocalnetRuntimeProvider basePath={environmentRoute?.basePath} environment={environment}>
      <StudioWorkspace
        environment={environment}
        environmentsState={environmentsState}
        route={route}
        onNavigate={navigate}
        onDeleteEnvironment={deleteEnvironment}
        onOpenEnvironment={openEnvironment}
        onSelectTestRun={selectTestRun}
      />
    </LocalnetRuntimeProvider>
  )
}

interface StudioWorkspaceProps {
  readonly environment?: StudioEnvironment
  readonly environmentsState: StudioEnvironmentsState
  readonly route: StudioRoute
  readonly onNavigate: (path: StudioPath) => void
  readonly onDeleteEnvironment: (environmentId: string) => void
  readonly onOpenEnvironment: (environment: StudioEnvironment) => void
  readonly onSelectTestRun: (runId: string | undefined, replace: boolean) => void
}

function StudioWorkspace({
  environment,
  environmentsState,
  route,
  onNavigate,
  onDeleteEnvironment,
  onOpenEnvironment,
  onSelectTestRun,
}: StudioWorkspaceProps) {
  const {showToast} = useToast()
  const location = useLocation()
  const runtime = useLocalnetRuntime()
  const activePath: StudioPath =
    route.kind === "page"
      ? route.path
      : route.kind === "test-run"
        ? "/tests"
        : route.section === "virtual-environments"
          ? "/virtual-environments"
          : "/"
  const [testSearchParams, setTestSearchParams] = useSearchParams()
  const selectedTestRunId = route.kind === "test-run" ? route.runId : undefined
  const selectedTestKey =
    route.kind === "test-run" ? (testSearchParams.get("test") ?? undefined) : undefined
  const hasSelectedTestRun = route.kind === "test-run"
  const selectTest = useCallback(
    (testKey: string, replace: boolean) => {
      setTestSearchParams(
        current => {
          const next = new URLSearchParams(current)
          next.set("test", testKey)
          return next
        },
        {replace},
      )
    },
    [setTestSearchParams],
  )
  const testRuns = useStudioTestRuns(
    activePath === "/" || activePath === "/tests",
    selectedTestRunId,
    onSelectTestRun,
  )
  const [studioInfo, setStudioInfo] = useState<StudioInfo>()
  const [connectionState, setConnectionState] = useState<StudioConnectionState>("connecting")
  const [isEnvironmentCreateOpen, setIsEnvironmentCreateOpen] = useState(false)
  const [isTestRunOpen, setIsTestRunOpen] = useState(false)
  const [sidebarMode, setSidebarMode] = useState<"studio" | "environment">(() =>
    route.kind === "environment" ? "environment" : "studio",
  )
  const [environmentShellState, setEnvironmentShellState] = useState<
    (LocalnetWorkspaceShellState & {readonly environmentId: string}) | undefined
  >()

  useEffect(() => {
    const controller = new AbortController()

    fetchStudioInfo(controller.signal)
      .then(info => {
        setStudioInfo(info)
        setConnectionState("connected")
      })
      .catch(error => {
        if (error instanceof DOMException && error.name === "AbortError") return
        setConnectionState("disconnected")
      })

    return () => controller.abort()
  }, [])

  useEffect(() => {
    const page = studioPages.find(candidate => candidate.path === activePath)
    document.title = page?.path === "/" ? "Acton Studio" : `${page?.label ?? "Studio"} · Acton`
  }, [activePath])

  useEffect(() => {
    if (route.kind !== "environment") setSidebarMode("studio")
  }, [route.kind])

  const navigate = useCallback(
    (path: StudioPath) => {
      if (path !== "/virtual-environments") setIsEnvironmentCreateOpen(false)
      if (path !== "/tests") setIsTestRunOpen(false)
      setSidebarMode("studio")
      onNavigate(path)
    },
    [onNavigate],
  )

  const openEnvironment = useCallback(
    (environment: StudioEnvironment) => {
      setIsEnvironmentCreateOpen(false)
      setSidebarMode("environment")
      onOpenEnvironment(environment)
    },
    [onOpenEnvironment],
  )

  const handleEnvironmentShellChange = useCallback(
    (state: LocalnetWorkspaceShellState) => {
      if (route.kind !== "environment") return

      setEnvironmentShellState(current => {
        if (
          current?.environmentId === route.environmentId &&
          current.pageDescription === state.pageDescription &&
          current.pageTitle === state.pageTitle &&
          current.primaryAction === state.primaryAction &&
          current.rpcUrl === state.rpcUrl
        ) {
          return current
        }

        return {
          ...state,
          environmentId: route.environmentId,
        }
      })
    },
    [route],
  )

  const showIntegrationToast = (action: string) => {
    showToast({
      title: `${action} is not connected yet`,
      description:
        "The Studio shell is ready; runtime integration is the next implementation step.",
      variant: "info",
    })
  }

  const environmentRoute = route.kind === "environment" ? route : undefined
  const managedEnvironments = environmentsState.environments.filter(
    environment => environment.lifecycle === "managed",
  )
  const externalNetworks = environmentsState.environments.filter(
    environment => environment.lifecycle === "external",
  )
  const isEnvironmentDashboard =
    environmentRoute !== undefined && location.pathname === `${environmentRoute.basePath}/dashboard`
  const environmentLoadError =
    environmentsState.error ??
    (environmentRoute && !environmentsState.isLoading && !environment
      ? environmentRoute.section === "networks"
        ? "Network not found"
        : "Virtual environment not found"
      : undefined)
  const activeFeaturePage =
    route.kind === "page" && activePath !== "/" ? studioFeaturePages[activePath] : undefined
  const ActiveFeatureIcon = activeFeaturePage?.icon
  const projectName = studioInfo?.workspace?.name ?? configuredProjectName
  const activeEnvironmentShell =
    environmentRoute && environmentShellState?.environmentId === environmentRoute.environmentId
      ? environmentShellState
      : undefined
  const showEnvironmentNavigation =
    Boolean(environmentRoute && environment) && sidebarMode === "environment"

  return (
    <StudioShell
      activePath={activePath}
      contentMode={
        isEnvironmentDashboard
          ? "workspace"
          : environmentRoute
            ? "full"
            : hasSelectedTestRun
              ? "workspace"
              : "default"
      }
      headerMode={hasSelectedTestRun ? "hidden" : "visible"}
      headerActions={
        environmentRoute ? (
          activeEnvironmentShell?.primaryAction ? (
            <Button
              variant="primary"
              size="sm"
              leadingIcon={
                activeEnvironmentShell.primaryAction.icon === "plus" ? (
                  <Plus size={16} aria-hidden="true" />
                ) : (
                  <Archive size={16} aria-hidden="true" />
                )
              }
              onClick={activeEnvironmentShell.primaryAction.onClick}
            >
              {activeEnvironmentShell.primaryAction.label}
            </Button>
          ) : activeEnvironmentShell?.rpcUrl ? (
            <CopyButton
              value={activeEnvironmentShell.rpcUrl}
              label="Copy RPC endpoint"
              copiedLabel="RPC endpoint copied"
              size="sm"
            >
              Copy RPC
            </CopyButton>
          ) : undefined
        ) : activePath === "/" ? (
          <>
            <Button
              variant="secondary"
              size="sm"
              leadingIcon={<FlaskConical size={16} aria-hidden="true" />}
              onClick={() => navigate("/tests")}
            >
              Tests
            </Button>
            <Button
              variant="primary"
              size="sm"
              leadingIcon={<Boxes size={16} aria-hidden="true" />}
              onClick={() => navigate("/virtual-environments")}
            >
              Virtual environments
            </Button>
          </>
        ) : activeFeaturePage && ActiveFeatureIcon ? (
          <Button
            variant="primary"
            size="sm"
            leadingIcon={<ActiveFeatureIcon size={16} aria-hidden="true" />}
            onClick={() => {
              if (activePath === "/virtual-environments") {
                setIsEnvironmentCreateOpen(true)
              } else if (activePath === "/tests") {
                setIsTestRunOpen(true)
              } else {
                showIntegrationToast(activeFeaturePage.actionLabel)
              }
            }}
          >
            {activeFeaturePage.actionLabel}
          </Button>
        ) : undefined
      }
      pageDescription={
        environmentRoute
          ? (activeEnvironmentShell?.pageDescription ??
            (environment?.lifecycle === "external"
              ? "Preparing this network"
              : "Preparing this virtual environment"))
          : undefined
      }
      pageTitle={
        environmentRoute
          ? (activeEnvironmentShell?.pageTitle ??
            environment?.name ??
            (environmentRoute.section === "networks" ? "Network" : "Virtual Environment"))
          : undefined
      }
      pages={studioPages}
      sidebarActiveEnvironmentId={
        environmentRoute?.section === "virtual-environments"
          ? environmentRoute.environmentId
          : undefined
      }
      sidebarActiveNetworkId={
        environmentRoute?.section === "networks" ? environmentRoute.environmentId : undefined
      }
      sidebarContextAction={
        environmentRoute && environment && !showEnvironmentNavigation
          ? {
              label: environment.name,
              onSelect: () => setSidebarMode("environment"),
            }
          : undefined
      }
      sidebarNavigation={
        showEnvironmentNavigation && environment ? (
          <EnvironmentNavigation
            environmentName={environment.name}
            onShowStudioNavigation={() => setSidebarMode("studio")}
          />
        ) : undefined
      }
      sidebarEnvironments={managedEnvironments}
      sidebarNetworks={externalNetworks}
      sidebarNavigationKey={
        showEnvironmentNavigation && environment ? `environment:${environment.id}` : "studio"
      }
      sidebarSearch={
        showEnvironmentNavigation ? <DashboardSearch client={runtime.client} /> : undefined
      }
      sidebarSelectedTestRunId={testRuns.selectedRunId}
      sidebarTestRuns={testRuns.runs}
      sidebarUtilityActions={
        showEnvironmentNavigation ? (
          <EnvironmentNavigationActions
            localnetApiToken={runtime.localnetApiToken}
            onOpenAuthTokenOverlay={runtime.openAuthOverlay}
          />
        ) : undefined
      }
      onNavigate={navigate}
      onOpenEnvironment={openEnvironment}
      onSelectTestRun={testRuns.selectRun}
    >
      {environmentRoute ? (
        <Suspense fallback={null}>
          <EnvironmentWorkspacePage
            basePath={environmentRoute.basePath}
            environment={environment}
            isLoading={environmentsState.isLoading}
            loadError={environmentLoadError}
            onEnvironmentChange={environmentsState.setEnvironment}
            onEnvironmentDelete={onDeleteEnvironment}
            onRetry={environmentsState.refresh}
            onShellChange={handleEnvironmentShellChange}
          />
        </Suspense>
      ) : activePath === "/" ? (
        <OverviewPage
          connectionState={connectionState}
          environments={managedEnvironments}
          environmentsError={environmentsState.error}
          environmentsLoading={environmentsState.isLoading}
          projectName={projectName}
          projectPath={configuredProjectPath}
          testRuns={testRuns.runs}
          testRunsError={testRuns.error}
          testRunsLoading={testRuns.isLoading}
          onNavigate={navigate}
          onOpenEnvironment={openEnvironment}
          onSelectTestRun={testRuns.selectRun}
        />
      ) : activePath === "/virtual-environments" ? (
        <VirtualEnvironmentsPage
          createOpen={isEnvironmentCreateOpen}
          environments={managedEnvironments}
          importSourceEnvironments={environmentsState.environments}
          isLoading={environmentsState.isLoading}
          loadError={environmentsState.error}
          walletNames={studioInfo?.workspace?.walletNames ?? []}
          onCreateOpenChange={setIsEnvironmentCreateOpen}
          onEnvironmentChange={environmentsState.setEnvironment}
          onOpenEnvironment={openEnvironment}
          onRefresh={environmentsState.refresh}
        />
      ) : activePath === "/tests" ? (
        <TestsPage
          runDialogOpen={isTestRunOpen}
          selectedTestKey={selectedTestKey}
          testRuns={testRuns}
          onSelectedTestKeyChange={selectTest}
          onRunDialogOpenChange={setIsTestRunOpen}
        />
      ) : (
        <FeaturePage page={studioFeaturePages[activePath]} onAction={showIntegrationToast} />
      )}
    </StudioShell>
  )
}

export function App() {
  return (
    <ToastProvider>
      <StudioApp />
    </ToastProvider>
  )
}
