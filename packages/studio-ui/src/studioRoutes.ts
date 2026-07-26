import {isStudioPath, type StudioPath} from "./studioPages"

const environmentPathPattern = /^\/virtual-environments\/([^/]+)(?:\/.*)?$/
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
    }
  | {
      readonly kind: "test-run"
      readonly runId: string
    }

export function readStudioRoute(pathname = globalThis.location.pathname): StudioRoute {
  const normalizedPath = pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname
  const environmentMatch = environmentPathPattern.exec(normalizedPath)

  if (environmentMatch) {
    try {
      const environmentId = decodeURIComponent(environmentMatch[1])
      return {
        basePath: environmentStudioPath(environmentId),
        environmentId,
        kind: "environment",
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

export function testRunStudioPath(runId: string) {
  return `/tests/${encodeURIComponent(runId)}`
}
