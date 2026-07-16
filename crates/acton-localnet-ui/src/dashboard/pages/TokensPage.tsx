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
} from "@acton/ui"
import {useNavigate} from "react-router-dom"
import {useEffect, useState} from "react"
import type {FC} from "react"

import type {TonClient} from "../../explorer/api/client"
import type {JettonMaster} from "../../explorer/api/types"
import {useDelayedLoadingVisibility} from "../../hooks/useDelayedLoadingVisibility"
import {AddressChip} from "../../explorer/components/AddressChip"
import {TOKEN_PLACEHOLDER_IMAGE} from "../constants"
import {formatTokenSupply} from "../dashboardUtils"

import styles from "../DashboardPage.module.css"

interface TokensPageProps {
  readonly client: TonClient
}

interface TokensState {
  readonly items: readonly JettonMaster[]
  readonly isLoading: boolean
  readonly error?: string
}

export const TokensPage: FC<TokensPageProps> = ({client}) => {
  const navigate = useNavigate()
  const [tokensState, setTokensState] = useState<TokensState>({
    items: [],
    isLoading: true,
  })
  const showLoadingSkeleton = useDelayedLoadingVisibility(tokensState.isLoading, 500)

  useEffect(() => {
    let cancelled = false

    void (async () => {
      setTokensState({
        items: [],
        isLoading: true,
      })

      try {
        const tokens = await client.getJettonMasters(undefined, 100, 0)
        if (cancelled) {
          return
        }
        setTokensState({
          items: tokens,
          isLoading: false,
        })
      } catch (error) {
        if (cancelled) {
          return
        }
        setTokensState({
          items: [],
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load tokens",
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [client])

  return (
    <>
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>Tokens</h1>
          <p className={styles.subtitle}>Jettons detected on the local network</p>
        </div>
      </section>

      <section
        className={styles.resourceTableLayout}
        aria-busy={tokensState.isLoading}
        aria-label={tokensState.isLoading ? "Loading tokens" : undefined}
      >
        <DataTable minWidth="50rem">
          <DataTableTable aria-label="Tokens" layout="fixed">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell>Token</DataTableHeaderCell>
                <DataTableHeaderCell align="right" columnWidth="12rem">
                  Supply
                </DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="8rem">Mintable</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="17rem">Address</DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {tokensState.error ? (
                <DataTableEmpty colSpan={4}>{tokensState.error}</DataTableEmpty>
              ) : tokensState.isLoading ? (
                showLoadingSkeleton ? (
                  <DataTableSkeletonRows
                    columns={4}
                    rows={3}
                    alignments={["left", "right", "left", "left"]}
                    widths={["14rem", "8rem", "4rem", "14rem"]}
                    rowKeyPrefix="token-table-skeleton"
                  />
                ) : null
              ) : tokensState.items.length === 0 ? (
                <DataTableEmpty colSpan={4}>No tokens yet</DataTableEmpty>
              ) : (
                tokensState.items.map(token => {
                  const symbol = token.jetton_content.symbol || "???"
                  const name = token.jetton_content.name || "Unknown Jetton"
                  const image = token.jetton_content.image || TOKEN_PLACEHOLDER_IMAGE
                  const href = `/explorer/address/${encodeURIComponent(token.address)}`

                  return (
                    <DataTableRow
                      key={token.address}
                      interactive
                      tabIndex={0}
                      onClick={() => {
                        void navigate(href)
                      }}
                      onKeyDown={event => {
                        if (event.target !== event.currentTarget) return
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault()
                          void navigate(href)
                        }
                      }}
                    >
                      <DataTableCell>
                        <div className={styles.assetTableIdentity}>
                          <img
                            src={image}
                            alt=""
                            className={styles.assetTableImage}
                            onError={event => {
                              const imageElement = event.currentTarget
                              if (imageElement.getAttribute("src") !== TOKEN_PLACEHOLDER_IMAGE) {
                                imageElement.src = TOKEN_PLACEHOLDER_IMAGE
                              }
                            }}
                          />
                          <span className={styles.assetTableText}>
                            <strong className={styles.assetTableName}>{name}</strong>
                            <span className={styles.assetTableSecondary}>{symbol}</span>
                          </span>
                        </div>
                      </DataTableCell>
                      <DataTableCell align="right" tone="strong">
                        {formatTokenSupply(token)}
                      </DataTableCell>
                      <DataTableCell>
                        <span
                          className={
                            token.mintable
                              ? styles.assetTableStatusPositive
                              : styles.assetTableStatusMuted
                          }
                        >
                          {token.mintable ? "Yes" : "No"}
                        </span>
                      </DataTableCell>
                      <DataTableCell>
                        <AddressChip address={token.address} resolveName={false} />
                      </DataTableCell>
                    </DataTableRow>
                  )
                })
              )}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
      </section>
    </>
  )
}
