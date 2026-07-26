import {FolderOpen} from "lucide-react"
import {useCallback, useEffect, useState} from "react"
import {useLocation, useNavigate} from "react-router"
import {Button, CopyButton, ToastProvider, useToast} from "@acton/ui"

import {StudioShell} from "./components/StudioShell"
import {useStudioEnvironment} from "./hooks/useStudioEnvironment"
import {EnvironmentNavigation} from "./localnet/dashboard/EnvironmentNavigation"
import {EnvironmentNavigationActions} from "./localnet/dashboard/EnvironmentNavigationActions"
import {DashboardSearch} from "./localnet/dashboard/DashboardSearch"
import type {LocalnetWorkspaceShellState} from "./localnet/LocalnetWorkspace"
import {LocalnetRuntimeProvider, useLocalnetRuntime} from "./localnet/LocalnetRuntimeProvider"
import {EnvironmentWorkspacePage} from "./pages/EnvironmentWorkspacePage"
import {FeaturePage} from "./pages/FeaturePage"
import {OverviewPage} from "./pages/OverviewPage"
import {VirtualEnvironmentsPage} from "./pages/VirtualEnvironmentsPage"
import {
  fetchStudioInfo,
  type StudioConnectionState,
  type StudioEnvironment,
  type StudioInfo,
} from "./studioApi"
import {studioFeaturePages, studioPages, type StudioPath} from "./studioPages"
import {environmentStudioPath, readStudioRoute, type StudioRoute} from "./studioRoutes"

const configuredProjectName = import.meta.env.VITE_STUDIO_PROJECT_NAME?.trim() || undefined
const configuredProjectPath = import.meta.env.VITE_STUDIO_PROJECT_PATH?.trim() || undefined

function StudioApp() {
  const location = useLocation()
  const routerNavigate = useNavigate()
  const route = readStudioRoute(location.pathname)
  const [selectedEnvironment, setSelectedEnvironment] = useState<StudioEnvironment>()
  const environmentRoute = route.kind === "environment" ? route : undefined
  const environmentState = useStudioEnvironment(
    environmentRoute?.environmentId,
    selectedEnvironment,
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
      setSelectedEnvironment(environment)
      void routerNavigate(environmentStudioPath(environment.id))
      globalThis.scrollTo({top: 0})
    },
    [routerNavigate],
  )

  return (
    <LocalnetRuntimeProvider
      basePath={environmentRoute?.basePath}
      environment={environmentState.environment}
    >
      <StudioWorkspace
        environmentState={environmentState}
        route={route}
        onNavigate={navigate}
        onOpenEnvironment={openEnvironment}
      />
    </LocalnetRuntimeProvider>
  )
}

interface StudioWorkspaceProps {
  readonly environmentState: ReturnType<typeof useStudioEnvironment>
  readonly route: StudioRoute
  readonly onNavigate: (path: StudioPath) => void
  readonly onOpenEnvironment: (environment: StudioEnvironment) => void
}

function StudioWorkspace({
  environmentState,
  route,
  onNavigate,
  onOpenEnvironment,
}: StudioWorkspaceProps) {
  const {showToast} = useToast()
  const runtime = useLocalnetRuntime()
  const [studioInfo, setStudioInfo] = useState<StudioInfo>()
  const [connectionState, setConnectionState] = useState<StudioConnectionState>("connecting")
  const [isEnvironmentCreateOpen, setIsEnvironmentCreateOpen] = useState(false)
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
    if (route.kind !== "page") return

    const activePath = route.path
    const page = studioPages.find(candidate => candidate.path === activePath)
    document.title = page?.path === "/" ? "Acton Studio" : `${page?.label ?? "Studio"} · Acton`
  }, [route])

  useEffect(() => {
    if (route.kind === "page") setSidebarMode("studio")
  }, [route.kind])

  const navigate = useCallback(
    (path: StudioPath) => {
      if (path !== "/virtual-environments") setIsEnvironmentCreateOpen(false)
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
  const environment = environmentState.environment
  const activePath: StudioPath = route.kind === "page" ? route.path : "/virtual-environments"
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
      contentMode={environmentRoute ? "full" : "default"}
      headerActions={
        environmentRoute ? (
          activeEnvironmentShell?.rpcUrl ? (
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
            <Button variant="secondary" size="sm" onClick={() => navigate("/virtual-environments")}>
              Explore workspace
            </Button>
            <Button
              variant="primary"
              size="sm"
              leadingIcon={<FolderOpen size={16} aria-hidden="true" />}
              onClick={() => showIntegrationToast("Project connection")}
            >
              Open project
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
          ? (activeEnvironmentShell?.pageDescription ?? "Preparing this virtual environment")
          : undefined
      }
      pageTitle={
        environmentRoute
          ? (activeEnvironmentShell?.pageTitle ?? environment?.name ?? "Virtual Environment")
          : undefined
      }
      pages={studioPages}
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
      sidebarNavigationKey={
        showEnvironmentNavigation && environment ? `environment:${environment.id}` : "studio"
      }
      sidebarSearch={
        showEnvironmentNavigation ? <DashboardSearch client={runtime.client} /> : undefined
      }
      sidebarUtilityActions={
        showEnvironmentNavigation ? (
          <EnvironmentNavigationActions
            localnetApiToken={runtime.localnetApiToken}
            onOpenAuthTokenOverlay={runtime.openAuthOverlay}
          />
        ) : undefined
      }
      onNavigate={navigate}
    >
      {environmentRoute ? (
        <EnvironmentWorkspacePage
          basePath={environmentRoute.basePath}
          environment={environment}
          isLoading={environmentState.isLoading}
          loadError={environmentState.error}
          onEnvironmentChange={environmentState.setEnvironment}
          onRetry={environmentState.refresh}
          onShellChange={handleEnvironmentShellChange}
        />
      ) : activePath === "/" ? (
        <OverviewPage
          connectionState={connectionState}
          pages={studioPages}
          projectName={projectName}
          projectPath={configuredProjectPath}
          onNavigate={navigate}
        />
      ) : activePath === "/virtual-environments" ? (
        <VirtualEnvironmentsPage
          createOpen={isEnvironmentCreateOpen}
          walletNames={studioInfo?.workspace?.walletNames ?? []}
          onCreateOpenChange={setIsEnvironmentCreateOpen}
          onOpenEnvironment={openEnvironment}
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
