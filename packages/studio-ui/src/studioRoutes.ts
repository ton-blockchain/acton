import {isStudioPath, type StudioPath} from "./studioPages"
import type {StudioEnvironment} from "./studioApi"

const managedEnvironmentPathPattern = /^\/virtual-environments\/([^/]+)(?:\/.*)?$/
const externalNetworkPathPattern = /^\/networks\/([^/]+)(?:\/.*)?$/
const testRunPathPattern = /^\/tests\/([^/]+)$/

export type StudioRoute =
  | {
      readonly kind: "page"
      readonly path: StudioPath
    }
  | {
      readonly basePath: string
      readonly environmentId: string
      readonly kind: "environment"
      readonly section: "virtual-environments" | "networks"
    }
  | {
      readonly kind: "test-run"
      readonly runId: string
    }

export function readStudioRoute(pathname = globalThis.location.pathname): StudioRoute {
  const normalizedPath = pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname
  const managedEnvironmentMatch = managedEnvironmentPathPattern.exec(normalizedPath)
  const externalNetworkMatch = externalNetworkPathPattern.exec(normalizedPath)
  const environmentMatch = managedEnvironmentMatch ?? externalNetworkMatch

  if (environmentMatch) {
    try {
      const environmentId = decodeURIComponent(environmentMatch[1])
      const section = managedEnvironmentMatch ? "virtual-environments" : "networks"
      return {
        basePath:
          section === "virtual-environments"
            ? environmentStudioPath(environmentId)
            : networkStudioPath(environmentId),
        environmentId,
        kind: "environment",
        section,
      }
    } catch {
      return {kind: "page", path: "/"}
    }
  }

  const testRunMatch = testRunPathPattern.exec(normalizedPath)
  if (testRunMatch) {
    try {
      return {
        kind: "test-run",
        runId: decodeURIComponent(testRunMatch[1]),
      }
    } catch {
      return {kind: "page", path: "/tests"}
    }
  }

  return {
    kind: "page",
    path: isStudioPath(normalizedPath) ? normalizedPath : "/",
  }
}

export function environmentStudioPath(environmentId: string) {
  return `/virtual-environments/${encodeURIComponent(environmentId)}`
}

export function networkStudioPath(networkId: string) {
  return `/networks/${encodeURIComponent(networkId)}`
}

export function studioEnvironmentPath(environment: StudioEnvironment) {
  return environment.lifecycle === "external"
    ? networkStudioPath(environment.id)
    : environmentStudioPath(environment.id)
}

export function testRunStudioPath(runId: string) {
  return `/tests/${encodeURIComponent(runId)}`
}
