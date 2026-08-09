import type {ParsedValue} from "@acton/ui"
import {readFile, writeFile} from "node:fs/promises"
import process from "node:process"

import {parseNetworkConfig} from "../src/api/config"

const DEFAULT_ENDPOINT = "https://toncenter.com/api/v2/getConfigAll"
const MANIFEST_URL = new URL("./mainnet-config-fields.json", import.meta.url)

export interface ConfigParameterShape {
  readonly id: number
  readonly fields: readonly string[]
  readonly parseError?: string
}

export interface MainnetConfigManifest {
  readonly network: "mainnet"
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
  manifest: MainnetConfigManifest,
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
  manifest: MainnetConfigManifest,
  parameters: readonly ConfigParameterShape[],
): MainnetConfigManifest {
  const merged: Record<string, readonly string[]> = {...manifest.parameters}

  for (const parameter of parameters) {
    if (parameter.parseError) continue

    const id = String(parameter.id)
    merged[id] = [...new Set([...(merged[id] ?? []), ...parameter.fields])].sort()
  }

  return {
    network: "mainnet",
    parameters: Object.fromEntries(
      Object.entries(merged).sort(([left], [right]) => Number(left) - Number(right)),
    ),
  }
}

async function main(): Promise<void> {
  const options = parseArguments(process.argv.slice(2))
  const [manifest, rawBoc] = await Promise.all([
    readManifest(),
    options.bocPath ? readFile(options.bocPath, "utf8") : fetchLatestMainnetConfig(),
  ])
  const parameters = inspectConfigBoc(rawBoc.trim())
  const additions = findConfigAdditions(manifest, parameters)

  if (options.update) {
    if (!hasConfigAdditions(additions)) {
      console.log(`Mainnet config is already covered (${parameters.length} parameters)`)
      return
    }
    if (Object.keys(additions.parseErrors).length > 0) {
      printAdditions(additions)
      throw new Error("Refusing to update the manifest until every positive parameter parses")
    }

    const updated = mergeConfigManifest(manifest, parameters)
    await writeFile(MANIFEST_URL, `${JSON.stringify(updated, null, 2)}\n`)
    console.log(`Updated ${MANIFEST_URL.pathname}`)
    return
  }

  if (hasConfigAdditions(additions)) {
    printAdditions(additions)
    process.exitCode = 1
    return
  }

  console.log(`Mainnet config is covered (${parameters.length} parameters)`)
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

async function readManifest(): Promise<MainnetConfigManifest> {
  return JSON.parse(await readFile(MANIFEST_URL, "utf8")) as MainnetConfigManifest
}

async function fetchLatestMainnetConfig(): Promise<string> {
  const endpoint = process.env.TONCENTER_MAINNET_CONFIG_URL?.trim() || DEFAULT_ENDPOINT
  const apiKey =
    process.env.TONCENTER_API_KEY?.trim() || process.env.EXPLORER_TONCENTER_API_KEY?.trim()
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

function parseArguments(arguments_: readonly string[]): {
  readonly bocPath?: string
  readonly update: boolean
} {
  let bocPath: string | undefined
  let update = false

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index]
    if (argument === "--update") {
      update = true
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

  return {bocPath, update}
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
