import {FolderOpen} from "lucide-react"
import {useCallback, useEffect, useState} from "react"
import {Button, ToastProvider, useToast} from "@acton/ui"

import {StudioShell} from "./components/StudioShell"
import {FeaturePage} from "./pages/FeaturePage"
import {OverviewPage} from "./pages/OverviewPage"
import {VirtualEnvironmentsPage} from "./pages/VirtualEnvironmentsPage"
import {fetchStudioInfo, type StudioConnectionState, type StudioInfo} from "./studioApi"
import {isStudioPath, studioFeaturePages, studioPages, type StudioPath} from "./studioPages"

const configuredProjectName = import.meta.env.VITE_STUDIO_PROJECT_NAME?.trim() || undefined
const configuredProjectPath = import.meta.env.VITE_STUDIO_PROJECT_PATH?.trim() || undefined
const trailingSlashesPattern = /\/+$/

function readPath(): StudioPath {
  const pathname =
    globalThis.location.pathname.length > 1
      ? globalThis.location.pathname.replace(trailingSlashesPattern, "")
      : globalThis.location.pathname

  return isStudioPath(pathname) ? pathname : "/"
}

function StudioApp() {
  const {showToast} = useToast()
  const [activePath, setActivePath] = useState<StudioPath>(readPath)
  const [studioInfo, setStudioInfo] = useState<StudioInfo>()
  const [connectionState, setConnectionState] = useState<StudioConnectionState>("connecting")
  const [isEnvironmentCreateOpen, setIsEnvironmentCreateOpen] = useState(false)

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
    const updatePath = () => setActivePath(readPath())
    globalThis.addEventListener("popstate", updatePath)
    return () => globalThis.removeEventListener("popstate", updatePath)
  }, [])

  useEffect(() => {
    const page = studioPages.find(candidate => candidate.path === activePath)
    document.title = page?.path === "/" ? "Acton Studio" : `${page?.label ?? "Studio"} · Acton`
  }, [activePath])

  const navigate = useCallback((path: StudioPath) => {
    if (globalThis.location.pathname !== path) {
      globalThis.history.pushState(null, "", path)
    }
    if (path !== "/virtual-environments") setIsEnvironmentCreateOpen(false)
    setActivePath(path)
    globalThis.scrollTo({top: 0})
  }, [])

  const showIntegrationToast = (action: string) => {
    showToast({
      title: `${action} is not connected yet`,
      description:
        "The Studio shell is ready; runtime integration is the next implementation step.",
      variant: "info",
    })
  }

  const activeFeaturePage = activePath === "/" ? undefined : studioFeaturePages[activePath]
  const ActiveFeatureIcon = activeFeaturePage?.icon
  const projectName = studioInfo?.workspace?.name ?? configuredProjectName

  return (
    <StudioShell
      activePath={activePath}
      headerActions={
        activePath === "/" ? (
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
      pages={studioPages}
      onNavigate={navigate}
    >
      {activePath === "/" ? (
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
