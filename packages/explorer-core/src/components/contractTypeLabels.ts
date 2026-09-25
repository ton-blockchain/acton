import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {humanizeIdentifier} from "@acton/ui"

export function getContractTypeLabels(
  compilerAbi?: ContractABI,
  interfaces?: readonly string[],
): string[] {
  const abiContractName = compilerAbi?.contract_name?.trim()
  const interfaceLabels = (interfaces ?? [])
    .map(value => getInterfaceLabel(value))
    .filter((value): value is string => value !== undefined)

  const labels = abiContractName ? [abiContractName, ...interfaceLabels] : interfaceLabels
  const seen = new Set<string>()
  const uniqueLabels = labels.filter(label => {
    const key = contractTypeLabelKey(label)
    if (seen.has(key)) {
      return false
    }
    seen.add(key)
    return true
  })
  return uniqueLabels.length > 0 ? uniqueLabels : ["Unknown"]
}

function contractTypeLabelKey(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "")
    .replace(/interface$/, "")
}

function getInterfaceLabel(value: string): string | undefined {
  const normalizedInterface = value.trim().toLowerCase()
  if (!normalizedInterface) {
    return undefined
  }

  switch (normalizedInterface) {
    case "jetton_master": {
      return "Jetton master interface"
    }
    case "jetton_wallet": {
      return "Jetton wallet interface"
    }
    case "nft_item":
    case "nft_item_simple": {
      return "NFT item interface"
    }
    case "nft_collection": {
      return "NFT collection interface"
    }
    case "multisig_v2": {
      return "Multisig wallet v2"
    }
    case "multisig_order_v2": {
      return "Multisig order v2"
    }
    default: {
      return humanizeIdentifier(normalizedInterface)
    }
  }
}
