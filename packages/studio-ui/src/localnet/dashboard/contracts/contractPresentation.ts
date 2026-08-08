import type {
  LocalnetContract,
  LocalnetContractSourceKind,
  LocalnetContractStatus,
} from "@acton/explorer-core/api/types"

export const contractStatusLabels = {
  active: "Active",
  frozen: "Frozen",
  uninitialized: "Uninitialized",
  nonexist: "Not deployed",
} satisfies Record<LocalnetContractStatus, string>

export const contractOriginLabels = {
  local: {short: "Local", detail: "Created locally"},
  fork: {short: "Fork", detail: "Fork state"},
  network: {short: "Network", detail: "Network state"},
} satisfies Record<LocalnetContractSourceKind, {readonly short: string; readonly detail: string}>

export interface ContractIdentity {
  readonly title: string
}

export function getContractIdentity(contract: LocalnetContract): ContractIdentity {
  const sourceName = getContractSourceName(contract)
  const customName = contract.name?.trim()

  return {
    title: customName || sourceName || "Unnamed contract",
  }
}

export function getContractSourceName(contract: LocalnetContract): string | undefined {
  return contract.abiName?.trim() || entrypointName(contract.artifact?.entrypoint)
}

export function entrypointName(entrypoint: string | undefined): string | undefined {
  return entrypoint
    ?.split("/")
    .at(-1)
    ?.replace(/\.(?:tolk|func?|fc)$/i, "")
}
