import {isStudioPath, type StudioPath} from "./studioPages"

const trailingSlashesPattern = /\/+$/
const environmentPathPattern = /^\/virtual-environments\/([^/]+)(?:\/.*)?$/

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

export function readStudioRoute(pathname = globalThis.location.pathname): StudioRoute {
  const normalizedPath =
    pathname.length > 1 ? pathname.replace(trailingSlashesPattern, "") : pathname
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

  return {
    kind: "page",
    path: isStudioPath(normalizedPath) ? normalizedPath : "/",
  }
}

export function environmentStudioPath(environmentId: string) {
  return `/virtual-environments/${encodeURIComponent(environmentId)}`
}
