import type {ValueFlowAssetMovement} from "@acton/transaction-ui"

import {addressKey} from "./compilerAbi"
import {getMetadataTokenInfo, metadataTokenDecimals, metadataTokenString} from "./tokenMetadata"
import type {V3Action, V3Metadata} from "./types"

export function buildActionValueFlowMovements(
  actions: readonly V3Action[],
  metadata: V3Metadata,
): ValueFlowAssetMovement[] {
  return actions.flatMap(action => {
    if (action.success !== true) {
      return []
    }

    const movement = (
      asset: string | null | undefined,
      amount: string | null | undefined,
      source?: string | null,
      destination?: string | null,
    ): ValueFlowAssetMovement[] => {
      const parsed = buildMovement(asset, amount, source, destination, metadata)
      return parsed ? [parsed] : []
    }

    switch (action.type) {
      case "jetton_transfer":
        return movement(
          action.details.asset,
          action.details.amount,
          action.details.sender,
          action.details.receiver,
        )
      case "jetton_mint":
        return movement(
          action.details.asset,
          action.details.amount,
          undefined,
          action.details.receiver,
        )
      case "jetton_burn":
        return movement(action.details.asset, action.details.amount, action.details.owner)
      case "jetton_swap":
      case "tonco_jetton_swap": {
        const incoming = action.details.dex_incoming_transfer
        const outgoing = action.details.dex_outgoing_transfer
        return [
          ...movement(incoming?.asset, incoming?.amount, incoming?.source, incoming?.destination),
          ...movement(outgoing?.asset, outgoing?.amount, outgoing?.source, outgoing?.destination),
        ]
      }
      default:
        return []
    }
  })
}

function buildMovement(
  assetAddress: string | null | undefined,
  amountValue: string | null | undefined,
  source: string | null | undefined,
  destination: string | null | undefined,
  metadata: V3Metadata,
): ValueFlowAssetMovement | undefined {
  if (!assetAddress || !amountValue || (!source && !destination)) {
    return undefined
  }

  let amount: bigint
  try {
    amount = BigInt(amountValue)
  } catch {
    return undefined
  }
  if (amount <= 0n) {
    return undefined
  }

  const tokenInfo = getMetadataTokenInfo(metadata, assetAddress, "jetton_masters")
  return {
    asset: {
      id: addressKey(assetAddress),
      symbol: metadataTokenString(tokenInfo, "symbol"),
      decimals: metadataTokenDecimals(tokenInfo),
    },
    source: source ?? undefined,
    destination: destination ?? undefined,
    amount,
  }
}
