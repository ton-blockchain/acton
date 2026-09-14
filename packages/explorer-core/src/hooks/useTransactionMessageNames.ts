import {useMemo} from "react"

import type {TonClient} from "../api/client"
import type {V3TransactionListItem} from "../api/types"

import {
  collectTransactionListAddresses,
  collectTransactionListAbiAddresses,
  type MessageNamesByAddress,
  useMessageNamesByAddress,
} from "./useMessageNamesByAddress"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"

export function useTransactionMessageNames(
  client: TonClient,
  transactions: readonly V3TransactionListItem[],
): {
  readonly addresses: readonly string[]
  readonly messageNamesByAddress: MessageNamesByAddress
} {
  const addresses = useMemo(() => collectTransactionListAddresses(transactions), [transactions])
  const abiAddresses = useMemo(
    () => collectTransactionListAbiAddresses(transactions),
    [transactions],
  )
  const metadataRegistry = useMetadataRegistry()
  const messageNamesByAddress = useMessageNamesByAddress({
    client,
    metadataRegistry,
    addresses: abiAddresses,
  })

  return {addresses, messageNamesByAddress}
}
