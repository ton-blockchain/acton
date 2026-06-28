import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {useEffect, useMemo, useState} from "react"

import type {TonClient} from "../api/client"
import {buildMessageNamesByOpcodeHex} from "../api/compilerAbi"
import {resolveCompilerAbis} from "../api/compilerAbiResolver"
import type {V3TransactionListItem} from "../api/types"
import type {ExplorerMetadataRegistry} from "../metadata/types"

export type MessageNamesByAddress = ReadonlyMap<
  string,
  {
    readonly incoming: ReadonlyMap<string, string>
    readonly outgoing: ReadonlyMap<string, string>
  }
>

/**
 * Inputs for resolving compiler ABI message names by account address.
 *
 * `addresses` should include every message endpoint whose ABI can affect
 * displayed transaction labels.
 */
interface UseMessageNamesByAddressOptions {
  /** Localnet client used to resolve account states and compiler ABIs. */
  readonly client: TonClient
  /** Product-specific metadata registry used to resolve compiler ABIs. */
  readonly metadataRegistry: ExplorerMetadataRegistry
  /** Raw or user-friendly addresses to resolve; duplicates are normalized away. */
  readonly addresses: readonly string[]
}

/**
 * Builds opcode-to-message-name maps for a set of account addresses.
 *
 * The returned map is keyed by a normalized raw address. Each value contains
 * incoming and outgoing opcode maps, so transaction renderers can resolve a
 * message name from either destination ABI or source ABI.
 */
export function useMessageNamesByAddress({
  client,
  metadataRegistry,
  addresses,
}: UseMessageNamesByAddressOptions): MessageNamesByAddress {
  // Keep this hook transaction-shape agnostic: callers collect the addresses
  // they care about, and this hook resolves those addresses to compiler ABIs.
  const requestedAddresses = useMemo(() => {
    return [...new Set(addresses.filter(Boolean))]
  }, [addresses])

  const [compilerAbiByAddress, setCompilerAbiByAddress] = useState<
    Map<string, ContractABI | undefined>
  >(new Map())

  useEffect(() => {
    let isActive = true

    const loadMessageNames = async () => {
      if (requestedAddresses.length === 0) {
        setCompilerAbiByAddress(new Map())
        return
      }

      const resolved = await resolveCompilerAbis({
        client,
        metadataRegistry,
        addresses: requestedAddresses,
        shouldContinue: () => isActive,
        onAccountStatesError: error => {
          console.error("Failed to fetch transaction account states", error)
        },
      })
      if (!resolved) {
        return
      }

      setCompilerAbiByAddress(new Map(resolved.abiByAddress))
    }

    void loadMessageNames()

    return () => {
      isActive = false
    }
  }, [client, metadataRegistry, requestedAddresses])

  return useMemo<MessageNamesByAddress>(() => {
    const next = new Map<
      string,
      {
        readonly incoming: ReadonlyMap<string, string>
        readonly outgoing: ReadonlyMap<string, string>
      }
    >()
    for (const [address, abi] of compilerAbiByAddress) {
      next.set(address, {
        incoming: buildMessageNamesByOpcodeHex(abi, "incoming_messages"),
        outgoing: buildMessageNamesByOpcodeHex(abi, "outgoing_messages"),
      })
    }
    return next
  }, [compilerAbiByAddress])
}

export function collectTransactionListAddresses(
  transactions: readonly V3TransactionListItem[],
): string[] {
  // V3 recent-transaction rows expose the account directly; include message
  // endpoints too, because opcode labels depend on source/destination ABIs.
  const addresses = new Set<string>()

  for (const transaction of transactions) {
    addresses.add(transaction.account)
    collectMessageAddresses(addresses, transaction.in_msg)
    for (const message of transaction.out_msgs) {
      collectMessageAddresses(addresses, message)
    }
  }

  return [...addresses]
}

function collectMessageAddresses(
  addresses: Set<string>,
  message: {readonly source?: string; readonly destination?: string} | null | undefined,
): void {
  if (message?.source) {
    addresses.add(message.source)
  }
  if (message?.destination) {
    addresses.add(message.destination)
  }
}
