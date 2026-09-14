import {Archive, FastForward, HandCoins, Pickaxe, Send, Settings2} from "lucide-react"
import {formatNumberValue, InlineButton, useToast} from "@acton/ui"
import {useCallback, useState} from "react"
import type {FC} from "react"

import {supports, supportsAny} from "../../../environmentCapabilities"
import {ImportAccountsButton} from "../../../components/ImportAccountsAction"
import type {StudioEnvironment} from "../../../studioApi"
import type {TonClient} from "@acton/explorer-core/api/client"

import styles from "../DashboardPage.module.css"

interface EnvironmentActionsProps {
  readonly client: TonClient
  readonly environment?: StudioEnvironment
  readonly isAdvanceTimeOpen: boolean
  readonly onAdvanceTime: () => void
  readonly onOpenMiningSettings: () => void
  readonly onFund: () => void
  readonly onSend: () => void
  readonly onSnapshots: () => void
  readonly onAdminActions: () => void
  readonly onStateChanged: () => void
}

export const EnvironmentActions: FC<EnvironmentActionsProps> = ({
  client,
  environment,
  isAdvanceTimeOpen,
  onAdvanceTime,
  onOpenMiningSettings,
  onFund,
  onSend,
  onSnapshots,
  onAdminActions,
  onStateChanged,
}) => {
  const {showToast} = useToast()
  const [busyAction, setBusyAction] = useState<string>()
  const hasFaucet = supportsAny(environment, "testnetFaucet", "gramFaucet", "jettonFaucet")
  const hasAccountActions = hasFaucet || supports(environment, "simulator")
  const hasRuntimeActions = supports(environment, "mining") || supports(environment, "timeTravel")
  const hasSnapshots = supports(environment, "snapshots")
  const hasAdminActions =
    environment?.config.kind === "fullTonNetwork" && environment.lifecycle === "managed"

  const mineBlock = useCallback(async () => {
    setBusyAction("mine-block")
    try {
      const result = await client.mineBlocks()
      if (result.blocks_mined > 0) {
        onStateChanged()
        showToast({
          variant: "success",
          title: "Block mined",
          description: `Block ${formatNumberValue(result.last_block_seqno)} is now the latest block`,
        })
      } else {
        showToast({
          variant: "info",
          title: "No block mined",
          description: (
            <span className={styles.toastDescriptionWithAction}>
              <span>There are no pending messages and empty block mining is disabled</span>
              <InlineButton variant="accent" onClick={onOpenMiningSettings}>
                Mining settings
              </InlineButton>
            </span>
          ),
          durationMs: 8000,
        })
      }
    } catch (error) {
      showToast({
        variant: "error",
        title: "Block not mined",
        description: errorMessage(error, "Failed to mine localnet block"),
      })
    } finally {
      setBusyAction(undefined)
    }
  }, [client, onOpenMiningSettings, onStateChanged, showToast])

  return (
    <>
      {hasAccountActions || hasRuntimeActions || hasSnapshots || hasAdminActions ? (
        <div className={styles.environmentActions}>
          {hasAccountActions ? (
            <div className={styles.environmentActionGroup} aria-label="Account actions">
              {hasFaucet ? (
                <InlineButton leadingIcon={<HandCoins size={15} />} onClick={onFund}>
                  Fund
                </InlineButton>
              ) : undefined}
              {supports(environment, "simulator") ? (
                <InlineButton leadingIcon={<Send size={15} />} onClick={onSend}>
                  Send
                </InlineButton>
              ) : undefined}
            </div>
          ) : undefined}

          {hasRuntimeActions ? (
            <div className={styles.environmentActionGroup} aria-label="Runtime actions">
              {supports(environment, "mining") ? (
                <InlineButton
                  leadingIcon={<Pickaxe size={15} />}
                  disabled={busyAction === "mine-block"}
                  onClick={() => void mineBlock()}
                >
                  {busyAction === "mine-block" ? "Mining" : "Mine block"}
                </InlineButton>
              ) : undefined}
              {supports(environment, "timeTravel") ? (
                <InlineButton
                  aria-haspopup="dialog"
                  aria-expanded={isAdvanceTimeOpen}
                  leadingIcon={<FastForward size={15} />}
                  onClick={onAdvanceTime}
                >
                  Advance time
                </InlineButton>
              ) : undefined}
            </div>
          ) : undefined}

          {hasSnapshots || hasAdminActions ? (
            <div className={styles.environmentActionGroup} aria-label="State actions">
              {hasAdminActions ? <ImportAccountsButton /> : undefined}
              {hasAdminActions ? (
                <InlineButton leadingIcon={<Settings2 size={15} />} onClick={onAdminActions}>
                  Admin actions
                </InlineButton>
              ) : undefined}
              {hasSnapshots ? (
                <InlineButton leadingIcon={<Archive size={15} />} onClick={onSnapshots}>
                  Snapshots
                </InlineButton>
              ) : undefined}
            </div>
          ) : undefined}
        </div>
      ) : undefined}
    </>
  )
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback
}
