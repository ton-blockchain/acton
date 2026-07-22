interface ApiReferenceSpec {
  readonly version: "control" | "v2" | "v3"
  readonly label: string
  readonly path: string
  readonly specUrl: string
  readonly fallbackTag: string
}

export interface ApiSearchIndexEntry {
  readonly id: string
  readonly title: string
  readonly description: string
  readonly href: string
  readonly searchText: string
}

const apiReferenceSpecs: readonly ApiReferenceSpec[] = [
  {
    version: "v2",
    label: "v2 API",
    path: "/api-reference/v2",
    specUrl: "/openapi/ton-api-v2.openapi.json",
    fallbackTag: "rpc",
  },
  {
    version: "v3",
    label: "v3 API",
    path: "/api-reference/v3",
    specUrl: "/openapi/ton-api-v3.openapi.json",
    fallbackTag: "actions",
  },
  {
    version: "control",
    label: "Control API",
    path: "/api-reference/control",
    specUrl: "/openapi/acton-localnet-control.openapi.json",
    fallbackTag: "Localnet Control",
  },
]

const httpMethods = new Set(["delete", "get", "head", "options", "patch", "post", "put", "trace"])

let cachedApiSearchIndex: Promise<readonly ApiSearchIndexEntry[]> | undefined

export function loadApiSearchIndex(): Promise<readonly ApiSearchIndexEntry[]> {
  if (!cachedApiSearchIndex) {
    cachedApiSearchIndex = Promise.all(
      apiReferenceSpecs.map(reference => loadApiReferenceSpec(reference)),
    )
      .then(entries => entries.flat())
      .catch(error => {
        cachedApiSearchIndex = undefined
        throw error
      })
  }

  return cachedApiSearchIndex
}

async function loadApiReferenceSpec(
  reference: ApiReferenceSpec,
): Promise<readonly ApiSearchIndexEntry[]> {
  const response = await fetch(reference.specUrl)
  if (!response.ok) {
    throw new Error(`Failed to load ${reference.label} spec: ${response.status}`)
  }

  const document: unknown = await response.json()
  return buildApiSearchIndex(reference, document)
}

function buildApiSearchIndex(
  reference: ApiReferenceSpec,
  document: unknown,
): readonly ApiSearchIndexEntry[] {
  if (!isRecord(document) || !isRecord(document.paths)) {
    return []
  }

  const entries: ApiSearchIndexEntry[] = []
  for (const [pathName, pathItem] of Object.entries(document.paths)) {
    if (!isRecord(pathItem)) {
      continue
    }

    for (const [methodName, operation] of Object.entries(pathItem)) {
      const method = methodName.toLocaleLowerCase()
      if (!httpMethods.has(method) || !isRecord(operation)) {
        continue
      }

      const methodLabel = method.toLocaleUpperCase()
      const operationId = readString(operation.operationId)
      const summary = readString(operation.summary)
      const description = readString(operation.description)
      const tag = readFirstString(operation.tags) ?? reference.fallbackTag
      const title = operationId ?? summary ?? `${methodLabel} ${pathName}`
      const resultDescription = `${reference.label} · ${methodLabel} ${pathName}${
        summary ? ` · ${summary}` : ""
      }`
      const searchFields = [
        title,
        summary,
        description,
        tag,
        reference.label,
        methodLabel,
        pathName,
        pathName.replace(/^\/api\/v[23]\//, ""),
      ].flatMap(value => (value ? [value] : []))

      entries.push({
        id: `api-${reference.version}-${method}-${pathName}`,
        title,
        description: resultDescription,
        href: `${reference.path}${apiReferenceAnchor(tag, methodLabel, pathName)}`,
        searchText: searchFields.join(" ").toLocaleLowerCase(),
      })
    }
  }

  return entries
}

function apiReferenceAnchor(tag: string, method: string, pathName: string): string {
  return `#tag/${slugifyTag(tag)}/${method}/${pathName.replace(/^\/+/, "")}`
}

function slugifyTag(tag: string): string {
  return tag
    .trim()
    .toLocaleLowerCase()
    .replaceAll(/[^a-z0-9]+/g, "-")
    .replaceAll(/^-+|-+$/g, "")
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined
}

function readFirstString(value: unknown): string | undefined {
  if (!Array.isArray(value)) {
    return undefined
  }

  return value.find((item): item is string => typeof item === "string" && item.length > 0)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}
