import type {ParsedValue} from "@acton/ui"
import {readFile, writeFile} from "node:fs/promises"
import process from "node:process"

import {parseNetworkConfig} from "../src/api/config"

export const NETWORKS = ["mainnet", "testnet"] as const
export type Network = (typeof NETWORKS)[number]

const NETWORK_SETTINGS: Readonly<
  Record<
    Network,
    {
      readonly endpoint: string
      readonly endpointEnvironmentVariable: string
      readonly apiKeyEnvironmentVariable: string
      readonly manifestUrl: URL
    }
  >
> = {
  mainnet: {
    endpoint: "https://toncenter.com/api/v2/getConfigAll",
    endpointEnvironmentVariable: "TONCENTER_MAINNET_CONFIG_URL",
    apiKeyEnvironmentVariable: "TONCENTER_MAINNET_API_KEY",
    manifestUrl: new URL("./config-fields/mainnet.json", import.meta.url),
  },
  testnet: {
    endpoint: "https://testnet.toncenter.com/api/v2/getConfigAll",
    endpointEnvironmentVariable: "TONCENTER_TESTNET_CONFIG_URL",
    apiKeyEnvironmentVariable: "TONCENTER_TESTNET_API_KEY",
    manifestUrl: new URL("./config-fields/testnet.json", import.meta.url),
  },
}

export interface ConfigParameterShape {
  readonly id: number
  readonly fields: readonly string[]
  readonly parseError?: string
}

export interface ConfigManifest {
  readonly network: Network
  readonly parameters: Readonly<Record<string, readonly string[]>>
}

export interface ConfigAdditions {
  readonly parameterIds: readonly number[]
  readonly fields: Readonly<Record<string, readonly string[]>>
  readonly parseErrors: Readonly<Record<string, string>>
}

export function inspectConfigBoc(rawBoc: string): readonly ConfigParameterShape[] {
  return parseNetworkConfig(rawBoc).parameters.map(parameter => ({
    id: parameter.id,
    fields: parameter.parsedValue ? collectFieldPaths(parameter.parsedValue) : [],
    ...(parameter.parseError === undefined ? {} : {parseError: parameter.parseError}),
  }))
}

export function findConfigAdditions(
  manifest: ConfigManifest,
  parameters: readonly ConfigParameterShape[],
): ConfigAdditions {
  const knownIds = new Set(Object.keys(manifest.parameters).map(Number))
  const parameterIds: number[] = []
  const fields: Record<string, readonly string[]> = {}
  const parseErrors: Record<string, string> = {}

  for (const parameter of parameters) {
    const id = String(parameter.id)
    if (!knownIds.has(parameter.id)) parameterIds.push(parameter.id)
    if (parameter.parseError) parseErrors[id] = parameter.parseError

    const knownFields = new Set(manifest.parameters[id] ?? [])
    const additions = parameter.fields.filter(field => !knownFields.has(field))
    if (additions.length > 0) fields[id] = additions
  }

  return {parameterIds, fields, parseErrors}
}

export function hasConfigAdditions(additions: ConfigAdditions): boolean {
  return (
    additions.parameterIds.length > 0 ||
    Object.keys(additions.fields).length > 0 ||
    Object.keys(additions.parseErrors).length > 0
  )
}

export function mergeConfigManifest(
  manifest: ConfigManifest,
  parameters: readonly ConfigParameterShape[],
): ConfigManifest {
  const merged: Record<string, readonly string[]> = {...manifest.parameters}

  for (const parameter of parameters) {
    if (parameter.parseError) continue

    const id = String(parameter.id)
    merged[id] = [...new Set([...(merged[id] ?? []), ...parameter.fields])].sort()
  }

  return {
    network: manifest.network,
    parameters: Object.fromEntries(
      Object.entries(merged).sort(([left], [right]) => Number(left) - Number(right)),
    ),
  }
}

async function main(): Promise<void> {
  const options = parseArguments(process.argv.slice(2))
  const networks = options.network ? [options.network] : NETWORKS
  for (const network of networks) await auditNetwork(network, options)
}

async function auditNetwork(
  network: Network,
  options: {readonly bocPath?: string; readonly fix: boolean},
): Promise<void> {
  const settings = NETWORK_SETTINGS[network]
  const [manifest, rawBoc] = await Promise.all([
    readManifest(network),
    options.bocPath ? readFile(options.bocPath, "utf8") : fetchLatestNetworkConfig(network),
  ])
  const parameters = inspectConfigBoc(rawBoc.trim())
  const additions = findConfigAdditions(manifest, parameters)

  if (options.fix) {
    if (!hasConfigAdditions(additions)) {
      console.log(
        `${capitalize(network)} config is already covered (${parameters.length} parameters)`,
      )
      return
    }
    if (Object.keys(additions.parseErrors).length > 0) {
      printAdditions(additions)
      throw new Error("Refusing to update the manifest until every config parameter parses")
    }

    const updated = mergeConfigManifest(manifest, parameters)
    await writeFile(settings.manifestUrl, `${JSON.stringify(updated, null, 2)}\n`)
    console.log(`Updated ${settings.manifestUrl.pathname}`)
    return
  }

  if (hasConfigAdditions(additions)) {
    printAdditions(additions)
    if (Object.keys(additions.parseErrors).length === 0) {
      console.error("\nRun `bun network-config:fix` to update the manifests")
    }
    process.exitCode = 1
    return
  }

  console.log(`${capitalize(network)} config is covered (${parameters.length} parameters)`)
}

function collectFieldPaths(value: ParsedValue): readonly string[] {
  const fields = new Set<string>()
  visitValue(value, "", fields)
  return [...fields].sort()
}

function visitValue(value: ParsedValue, prefix: string, fields: Set<string>): void {
  if (value.kind === "array") {
    const itemPrefix = prefix ? `${prefix}[]` : "$array[]"
    fields.add(itemPrefix)
    for (const item of value.items) visitValue(item, itemPrefix, fields)
    return
  }

  if (value.kind === "map") {
    const itemPrefix = prefix ? `${prefix}{}` : "$map{}"
    fields.add(itemPrefix)
    for (const entry of value.entries) visitValue(entry.value, itemPrefix, fields)
    return
  }

  if (value.kind !== "object") {
    if (!prefix) fields.add("$value")
    return
  }

  if (value.entries.length === 0 && !prefix) fields.add("$value")
  for (const entry of value.entries) {
    const key = normalizeFieldName(entry.key)
    const path = prefix ? `${prefix}.${key}` : key
    fields.add(path)
    visitValue(entry.value, path, fields)
  }
}

function normalizeFieldName(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, "_")
    .replaceAll(/^_+|_+$/g, "")
}

async function readManifest(network: Network): Promise<ConfigManifest> {
  const manifest = JSON.parse(
    await readFile(NETWORK_SETTINGS[network].manifestUrl, "utf8"),
  ) as ConfigManifest
  if (manifest.network !== network) {
    throw new Error(`Expected ${network} manifest, received ${manifest.network}`)
  }
  return manifest
}

async function fetchLatestNetworkConfig(network: Network): Promise<string> {
  const settings = NETWORK_SETTINGS[network]
  const endpoint = process.env[settings.endpointEnvironmentVariable]?.trim() || settings.endpoint
  const apiKey =
    process.env[settings.apiKeyEnvironmentVariable]?.trim() ||
    process.env.TONCENTER_API_KEY?.trim() ||
    process.env.EXPLORER_TONCENTER_API_KEY?.trim()
  const response = await fetch(endpoint, {
    headers: apiKey ? {"X-API-Key": apiKey} : undefined,
    signal: AbortSignal.timeout(30_000),
  })
  if (!response.ok) throw new Error(`Toncenter returned HTTP ${response.status}`)

  const payload = (await response.json()) as {
    readonly ok?: boolean
    readonly error?: string
    readonly result?: {readonly config?: {readonly bytes?: string}}
  }
  const bytes = payload.result?.config?.bytes
  if (payload.ok !== true || !bytes) {
    throw new Error(payload.error || "Toncenter response contains no config BOC")
  }
  return bytes
}

export function parseArguments(arguments_: readonly string[]): {
  readonly network?: Network
  readonly bocPath?: string
  readonly fix: boolean
} {
  let network: Network | undefined
  let bocPath: string | undefined
  let fix = false

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index]
    if (argument === "--fix") {
      fix = true
      continue
    }
    if (argument === "--network") {
      const value = arguments_[index + 1]
      if (!isNetwork(value)) throw new Error("--network must be mainnet or testnet")
      network = value
      index += 1
      continue
    }
    if (argument === "--boc") {
      bocPath = arguments_[index + 1]
      if (!bocPath) throw new Error("--boc requires a file path")
      index += 1
      continue
    }
    throw new Error(`Unknown argument: ${argument}`)
  }

  if (bocPath && !network) throw new Error("--boc requires --network")
  return {network, bocPath, fix}
}

function isNetwork(value: string | undefined): value is Network {
  return NETWORKS.some(network => network === value)
}

function capitalize(value: string): string {
  return `${value.charAt(0).toUpperCase()}${value.slice(1)}`
}

function printAdditions(additions: ConfigAdditions): void {
  for (const id of additions.parameterIds) console.error(`+ ConfigParam ${id}`)
  for (const [id, fields] of Object.entries(additions.fields)) {
    for (const field of fields) console.error(`+ ConfigParam ${id}.${field}`)
  }
  for (const [id, error] of Object.entries(additions.parseErrors)) {
    console.error(`! ConfigParam ${id} does not parse: ${error}`)
  }
}

if (import.meta.main) await main()
