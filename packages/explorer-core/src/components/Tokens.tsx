import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  TokenAmount,
} from "@acton/ui"
import {useEffect, useState} from "react"
import type {FC} from "react"

import type {TonClient} from "../api/client"
import type {JettonMasterMetadata, JettonWallet} from "../api/types"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"

import {ExplorerAddressChip} from "./ExplorerAddressChip"
import styles from "./Tokens.module.css"
import {
  TOKEN_IMAGE_SOURCE_KEYS,
  getImageSources,
  getPrimaryImageSource,
  replaceBrokenImageWithFallback,
} from "./imageFallbacks"
import {toRawAddress} from "./utils"

interface TokensProps {
  readonly wallets: JettonWallet[]
  readonly client: TonClient
  readonly onAddressClick?: (addr: string, event?: ExplorerNavigationClickEvent) => void
}

export const Tokens: FC<TokensProps> = ({wallets, client, onAddressClick}) => {
  const [mastersByAddress, setMastersByAddress] = useState<Map<string, JettonMasterMetadata>>(
    () => new Map(),
  )

  useEffect(() => {
    let isActive = true

    const fetchMasters = async () => {
      const inlineMasters = new Map<string, JettonMasterMetadata>()
      const missingMasterAddresses = new Set<string>()

      for (const wallet of wallets) {
        const key = toRawAddress(wallet.jetton)
        if (wallet.master) {
          inlineMasters.set(key, wallet.master)
        } else {
          missingMasterAddresses.add(wallet.jetton)
        }
      }

      setMastersByAddress(inlineMasters)
      if (missingMasterAddresses.size === 0) {
        return
      }

      try {
        const masters = await client.getJettonMasters([...missingMasterAddresses])
        if (!isActive) return
        setMastersByAddress(
          new Map([
            ...inlineMasters,
            ...masters.map(master => [toRawAddress(master.address), master] as const),
          ]),
        )
      } catch (error) {
        console.error("Failed to fetch jetton masters", error)
      }
    }

    void fetchMasters()
    return () => {
      isActive = false
    }
  }, [wallets, client])

  return (
    <DataTable className={styles.embeddedTable} minWidth="54rem">
      <DataTableTable aria-label="Tokens" layout="fixed">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell>Token</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="22rem">Amount</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="18rem">
              Wallet address
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {wallets.length === 0 ? (
            <DataTableEmpty colSpan={3}>No tokens found</DataTableEmpty>
          ) : (
            wallets.map(wallet => {
              const master = wallet.master ?? mastersByAddress.get(toRawAddress(wallet.jetton))
              const symbol = master?.jetton_content?.symbol || "UNKNOWN"
              const name = master?.jetton_content?.name || "Unknown Jetton"
              const imageSources = getImageSources(master?.jetton_content, TOKEN_IMAGE_SOURCE_KEYS)
              const image = getPrimaryImageSource(master?.jetton_content, TOKEN_IMAGE_SOURCE_KEYS)

              return (
                <DataTableRow
                  key={wallet.address}
                  interactive={Boolean(onAddressClick)}
                  tabIndex={onAddressClick ? 0 : undefined}
                  onClick={event => onAddressClick?.(wallet.jetton, event)}
                  onKeyDown={
                    onAddressClick
                      ? event => {
                          if (event.target !== event.currentTarget) return
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault()
                            event.currentTarget.click()
                          }
                        }
                      : undefined
                  }
                >
                  <DataTableCell>
                    <div className={styles.tokenIdentity}>
                      <img
                        src={image}
                        alt=""
                        className={styles.jettonImage}
                        onError={event => replaceBrokenImageWithFallback(event, imageSources)}
                      />
                      <strong className={styles.jettonName} title={name}>
                        {name}
                      </strong>
                    </div>
                  </DataTableCell>
                  <DataTableCell tone="strong">
                    <TokenAmount
                      className={styles.amount}
                      decimals={master?.jetton_content.decimals}
                      symbol={symbol}
                      useGrouping
                      value={wallet.balance}
                    />
                  </DataTableCell>
                  <DataTableCell align="right">
                    <ExplorerAddressChip
                      address={wallet.address}
                      copyPlacement="left"
                      onAddressClick={onAddressClick}
                      resolveName={false}
                    />
                  </DataTableCell>
                </DataTableRow>
              )
            })
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

export const TokensSkeleton: FC = () => {
  return (
    <DataTable className={styles.embeddedTable} minWidth="54rem">
      <DataTableTable aria-label="Loading tokens" layout="fixed">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell>Token</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="22rem">Amount</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="18rem">
              Wallet address
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          <DataTableSkeletonRows
            columns={3}
            rows={5}
            alignments={["left", "left", "right"]}
            widths={["14rem", "8rem", "14rem"]}
            rowKeyPrefix="token-table-skeleton"
          />
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}
