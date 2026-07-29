import {Button, Checkbox, CopyInlineButton, Input, Tooltip, useToast} from "@acton/ui"
import {CircleHelp, Trash2} from "lucide-react"
import {useCallback, useEffect, useState} from "react"
import type {FC} from "react"

import {supports} from "../../../environmentCapabilities"
import {deleteStudioEnvironment, updateStudioEnvironment} from "../../../studioApi"
import type {StudioEnvironment} from "../../../studioApi"
import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"
import type {TonClient} from "@acton/explorer-core/api/client"
import type {LocalnetMiningMode} from "@acton/explorer-core/api/types"
import {DeleteEnvironmentDialog} from "../components/DeleteEnvironmentDialog"

import styles from "../DashboardPage.module.css"

interface SettingsPageProps {
  readonly client: TonClient
  readonly onEnvironmentChange: (environment: StudioEnvironment) => void
  readonly onEnvironmentDelete: (environmentId: string) => void
}

export const SettingsPage: FC<SettingsPageProps> = ({
  client,
  onEnvironmentChange,
  onEnvironmentDelete,
}) => {
  const {showToast} = useToast()
  const {environment} = useLocalnetRuntime()
  const localnetConfig =
    environment?.config.kind === "actonLocalnet" ? environment.config : undefined
  const fullNetworkConfig =
    environment?.config.kind === "fullTonNetwork" ? environment.config : undefined
  const hasControlApi = supports(environment, "controlApi")
  const hasMining = supports(environment, "mining")
  const [miningMode, setMiningMode] = useState<LocalnetMiningMode>()
  const [autoMining, setAutoMining] = useState<boolean>()
  const [blockIntervalMs, setBlockIntervalMs] = useState<number>()
  const [rateLimitRps, setRateLimitRps] = useState<number | null>()
  const [environmentName, setEnvironmentName] = useState(environment?.name ?? "")
  const [responseDelay, setResponseDelay] = useState("")
  const [isLoading, setIsLoading] = useState(true)
  const [savingAction, setSavingAction] = useState<string>()
  const [loadError, setLoadError] = useState<string>()
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)

  const loadRuntimeSettings = useCallback(async () => {
    if (!hasControlApi) {
      setMiningMode(undefined)
      setAutoMining(undefined)
      setBlockIntervalMs(undefined)
      setRateLimitRps(undefined)
      setResponseDelay("")
      setLoadError(undefined)
      setIsLoading(false)
      return
    }

    setIsLoading(true)
    setLoadError(undefined)
    try {
      const nodeInfo = await client.getNodeInfo()
      setMiningMode(nodeInfo.mining_mode)
      setAutoMining(nodeInfo.auto_mining)
      setBlockIntervalMs(nodeInfo.block_interval_ms)
      setRateLimitRps(nodeInfo.rate_limit_rps)
      setResponseDelay((nodeInfo.network_conditions?.response_delay_ms ?? 0).toString())
    } catch (error) {
      setLoadError(errorMessage(error, "Failed to load runtime settings"))
    } finally {
      setIsLoading(false)
    }
  }, [client, hasControlApi])

  useEffect(() => {
    void loadRuntimeSettings()
  }, [loadRuntimeSettings])

  useEffect(() => {
    setEnvironmentName(environment?.name ?? "")
  }, [environment?.name])

  const saveEnvironmentName = useCallback(async () => {
    if (!environment) return
    const name = environmentName.trim()
    if (!name || name === environment.name) return

    setSavingAction("name")
    try {
      const updatedEnvironment = await updateStudioEnvironment(environment.id, {name})
      onEnvironmentChange(updatedEnvironment)
      setEnvironmentName(updatedEnvironment.name)
      showToast({
        variant: "success",
        title: "Environment renamed",
        description: `The environment is now named ${updatedEnvironment.name}`,
      })
    } catch (error) {
      showToast({
        variant: "error",
        title: "Environment not renamed",
        description: errorMessage(error, "Failed to update the environment name"),
      })
    } finally {
      setSavingAction(undefined)
    }
  }, [environment, environmentName, onEnvironmentChange, showToast])

  const saveResponseDelay = useCallback(async () => {
    const delay = parseResponseDelay(responseDelay)
    if (delay === undefined) return

    setSavingAction("response-delay")
    try {
      const conditions = await client.setNetworkConditions(delay)
      setResponseDelay(conditions.response_delay_ms.toString())
      showToast({
        variant: "success",
        title: "Network conditions updated",
        description:
          conditions.response_delay_ms === 0
            ? "Artificial response delay is disabled"
            : `Responses are delayed by ${conditions.response_delay_ms.toLocaleString()} ms`,
      })
    } catch (error) {
      showToast({
        variant: "error",
        title: "Network conditions not updated",
        description: errorMessage(error, "Failed to update network conditions"),
      })
    } finally {
      setSavingAction(undefined)
    }
  }, [client, responseDelay, showToast])

  const updateEmptyBlockMining = useCallback(
    async (mineEmptyBlocks: boolean) => {
      if (!miningMode) return

      const previousMode = miningMode
      const nextMode = {skip_empty_blocks: !mineEmptyBlocks}
      setMiningMode(nextMode)
      setSavingAction("empty-blocks")
      try {
        setMiningMode(await client.setMiningMode(nextMode.skip_empty_blocks))
        showToast({
          variant: "success",
          title: "Mining settings updated",
          description: mineEmptyBlocks
            ? "Manual and automatic mining can now create empty blocks"
            : "Blocks without pending messages will now be skipped",
        })
      } catch (error) {
        setMiningMode(previousMode)
        showToast({
          variant: "error",
          title: "Mining settings not updated",
          description: errorMessage(error, "Failed to update mining settings"),
        })
      } finally {
        setSavingAction(undefined)
      }
    },
    [client, miningMode, showToast],
  )

  const deleteEnvironment = useCallback(async () => {
    if (!environment) return
    setIsDeleting(true)
    try {
      await deleteStudioEnvironment(environment.id)
      showToast({
        variant: "success",
        title: "Environment deleted",
        description: `${environment.name} and its stored state were removed`,
      })
      onEnvironmentDelete(environment.id)
    } catch (error) {
      showToast({
        variant: "error",
        title: "Environment not deleted",
        description: errorMessage(error, "Failed to delete the environment"),
      })
    } finally {
      setIsDeleting(false)
    }
  }, [environment, onEnvironmentDelete, showToast])

  const parsedResponseDelay = parseResponseDelay(responseDelay)
  const endpointRows = environment
    ? [
        environment.endpoints.apiV2
          ? {label: "V2 API endpoint", value: absoluteUrl(environment.endpoints.apiV2)}
          : undefined,
        environment.endpoints.apiV3
          ? {label: "V3 API endpoint", value: absoluteUrl(environment.endpoints.apiV3)}
          : undefined,
        environment.endpoints.control
          ? {label: "Control API endpoint", value: absoluteUrl(environment.endpoints.control)}
          : undefined,
      ].filter((row): row is {readonly label: string; readonly value: string} => Boolean(row))
    : []
  const nameCanBeSaved =
    environment !== undefined &&
    environmentName.trim().length > 0 &&
    environmentName.trim() !== environment.name

  return (
    <div className={styles.settingsPage}>
      <section className={styles.settingsSection} aria-labelledby="general-settings-title">
        <SettingsSectionHeader
          id="general-settings-title"
          title="General"
          description="Change how this environment is named and identified"
        />

        <div className={styles.settingsRows}>
          <div className={styles.settingsRow}>
            <div className={styles.settingsRowCopy}>
              <strong>Name</strong>
              <span>The name shown for this environment throughout Studio</span>
            </div>
            <div className={styles.settingsEditableControl}>
              <Input
                aria-label="Environment name"
                size="sm"
                maxLength={80}
                value={environmentName}
                invalid={environmentName.trim().length === 0}
                onChange={event => setEnvironmentName(event.target.value)}
                onKeyDown={event => {
                  if (event.key === "Enter") void saveEnvironmentName()
                }}
              />
              <Button
                size="sm"
                variant="secondary"
                loading={savingAction === "name"}
                disabled={!nameCanBeSaved}
                onClick={() => void saveEnvironmentName()}
              >
                Save
              </Button>
            </div>
          </div>

          <SettingsValueRow
            label="Environment ID"
            description="A unique identifier for this environment"
            value={environment?.id ?? "Unavailable"}
            technical
          />
        </div>
      </section>

      <section className={styles.settingsSection} aria-labelledby="network-settings-title">
        <SettingsSectionHeader
          id="network-settings-title"
          title="Network"
          description="Review connection details and adjust network behavior"
        />

        <div className={styles.settingsRows}>
          <SettingsValueRow
            label="Environment type"
            description="The runtime used by this environment"
            value={
              environment?.config.kind === "fullTonNetwork"
                ? "Full TON network"
                : environment?.config.kind === "remoteTonNetwork"
                  ? "Remote TON network"
                  : "Fast local network"
            }
          />

          <SettingsValueRow
            label="Network"
            description="The TON network identity exposed to connected tools"
            value={environment?.network.label ?? "Unavailable"}
          />

          {endpointRows.map(endpoint => (
            <SettingsValueRow
              key={endpoint.label}
              label={endpoint.label}
              description="Connect compatible tools to this environment"
              value={endpoint.value}
              copyValue={endpoint.value}
              technical
            />
          ))}

          {endpointRows.length === 0 ? (
            <SettingsValueRow
              label="Endpoints"
              description="Connections become available when the environment is running"
              value="Unavailable"
            />
          ) : undefined}

          {localnetConfig ? (
            <>
              <SettingsValueRow
                label="Runtime port"
                description="The port assigned to this environment"
                value={localnetConfig.port?.toString() ?? "Automatic"}
                copyValue={localnetConfig.port?.toString()}
                technical
              />

              <SettingsValueRow
                label="Initial state"
                description="Network state loaded when this environment was created"
                value={formatForkState(environment)}
              />
            </>
          ) : undefined}

          {fullNetworkConfig ? (
            <SettingsValueRow
              label="Validators"
              description="Validator nodes configured for this network"
              value={(fullNetworkConfig.validators ?? 1).toLocaleString()}
            />
          ) : undefined}

          {hasControlApi ? (
            <>
              <div className={styles.settingsRow}>
                <div className={styles.settingsRowCopy}>
                  <strong>Response delay</strong>
                  <span>Simulate slower network responses, or use 0 to disable the delay</span>
                </div>
                <div className={styles.settingsNumberControl}>
                  <Input
                    aria-label="Response delay"
                    type="number"
                    inputMode="numeric"
                    min={0}
                    max={60_000}
                    size="sm"
                    suffix="ms"
                    value={responseDelay}
                    invalid={responseDelay.length > 0 && parsedResponseDelay === undefined}
                    disabled={isLoading}
                    onChange={event => setResponseDelay(event.target.value)}
                    onKeyDown={event => {
                      if (event.key === "Enter") void saveResponseDelay()
                    }}
                  />
                  <Button
                    size="sm"
                    variant="secondary"
                    loading={savingAction === "response-delay"}
                    disabled={parsedResponseDelay === undefined || isLoading}
                    onClick={() => void saveResponseDelay()}
                  >
                    Apply
                  </Button>
                </div>
              </div>

              <SettingsValueRow
                label="Rate limit"
                description="Maximum number of requests this environment accepts per second"
                value={
                  isLoading
                    ? "Loading"
                    : rateLimitRps === undefined
                      ? "Unavailable"
                      : rateLimitRps === null
                        ? "Unlimited"
                        : `${rateLimitRps.toLocaleString()} requests/s`
                }
              />
            </>
          ) : undefined}

          {localnetConfig && supports(environment, "wallets") ? (
            <div className={styles.settingsRow}>
              <div className={styles.settingsRowCopy}>
                <strong>Startup accounts</strong>
                <span>Wallets available as soon as this environment starts</span>
              </div>
              <div className={styles.settingsBadges}>
                {localnetConfig.accounts.length > 0 ? (
                  localnetConfig.accounts.map(account => (
                    <span key={account} className={styles.settingsBadge}>
                      {account}
                    </span>
                  ))
                ) : (
                  <span className={styles.settingsValueMuted}>None</span>
                )}
              </div>
            </div>
          ) : undefined}
        </div>
      </section>

      {hasMining ? (
        <section className={styles.settingsSection} aria-labelledby="mining-settings-title">
          <SettingsSectionHeader
            id="mining-settings-title"
            title="Mining"
            description="Review how this environment creates new blocks"
          />

          <div className={styles.settingsRows}>
            <SettingsValueRow
              label="Automatic mining"
              description="Create blocks automatically while the environment is running"
              value={
                isLoading
                  ? "Loading"
                  : autoMining === undefined
                    ? "Unavailable"
                    : autoMining
                      ? "Enabled"
                      : "Disabled"
              }
            />

            <SettingsValueRow
              label="Block interval"
              description="Time between automatic block creation attempts"
              value={
                isLoading
                  ? "Loading"
                  : autoMining === false
                    ? "Not applicable"
                    : blockIntervalMs === undefined
                      ? "Unavailable"
                      : `${blockIntervalMs.toLocaleString()} ms`
              }
            />

            <div className={styles.settingsRow}>
              <div className={styles.settingsRowCopy}>
                <strong>Empty blocks</strong>
                <span>Create blocks even when there are no pending messages</span>
              </div>
              {isLoading ? (
                <span className={styles.settingsValueMuted}>Loading</span>
              ) : loadError ? (
                <Button size="sm" variant="outline" onClick={() => void loadRuntimeSettings()}>
                  Retry
                </Button>
              ) : (
                <Checkbox
                  label="Mine empty blocks"
                  checked={!miningMode?.skip_empty_blocks}
                  disabled={savingAction === "empty-blocks"}
                  onChange={event => void updateEmptyBlockMining(event.target.checked)}
                />
              )}
            </div>

            {loadError ? (
              <div className={styles.settingsInlineError} role="alert">
                {loadError}
              </div>
            ) : undefined}
          </div>
        </section>
      ) : undefined}

      <section className={styles.settingsSection} aria-labelledby="danger-settings-title">
        <SettingsSectionHeader
          danger
          id="danger-settings-title"
          title="Danger zone"
          description="Delete this environment and its saved data"
        />

        <div className={styles.settingsRows}>
          <div className={styles.settingsRow}>
            <div className={styles.settingsRowCopy}>
              <strong>Delete environment</strong>
              <span>Permanently delete this environment and all saved data</span>
            </div>
            <Button
              size="sm"
              variant="danger"
              leadingIcon={<Trash2 size={15} aria-hidden="true" />}
              onClick={() => setIsDeleteDialogOpen(true)}
            >
              Delete environment
            </Button>
          </div>
        </div>
      </section>

      <DeleteEnvironmentDialog
        environment={isDeleteDialogOpen ? environment : undefined}
        loading={isDeleting}
        onConfirm={() => void deleteEnvironment()}
        onOpenChange={setIsDeleteDialogOpen}
      />
    </div>
  )
}

function parseResponseDelay(value: string): number | undefined {
  const delay = Number(value)
  return Number.isInteger(delay) && delay >= 0 && delay <= 60_000 ? delay : undefined
}

function formatForkState(environment?: StudioEnvironment): string {
  if (environment?.config.kind !== "actonLocalnet") return "Not applicable"

  const network = environment.config.forkNetwork
  if (!network) return "Clean network"

  const block = environment.config.forkBlockNumber
  return block ? `${network} at block ${block.toLocaleString()}` : `${network} latest`
}

function absoluteUrl(value: string): string {
  try {
    return new URL(value, globalThis.location.origin).href
  } catch {
    return value
  }
}

interface SettingsSectionHeaderProps {
  readonly danger?: boolean
  readonly description: string
  readonly id: string
  readonly title: string
}

const SettingsSectionHeader: FC<SettingsSectionHeaderProps> = ({
  danger = false,
  description,
  id,
  title,
}) => (
  <header className={styles.settingsSectionHeader}>
    <h2 id={id} className={danger ? styles.settingsDangerTitle : undefined}>
      {title}
    </h2>
    <Tooltip content={description}>
      <button type="button" className={styles.settingsSectionHelp} aria-label={`About ${title}`}>
        <CircleHelp size={14} aria-hidden="true" />
      </button>
    </Tooltip>
  </header>
)

interface SettingsValueRowProps {
  readonly copyValue?: string
  readonly description: string
  readonly label: string
  readonly technical?: boolean
  readonly value: string
}

const SettingsValueRow: FC<SettingsValueRowProps> = ({
  copyValue,
  description,
  label,
  technical = false,
  value,
}) => (
  <div className={styles.settingsRow}>
    <div className={styles.settingsRowCopy}>
      <strong>{label}</strong>
      <span>{description}</span>
    </div>
    {copyValue ? (
      <div className={styles.settingsValueWithAction}>
        <span className={technical ? styles.settingsCodeValue : styles.settingsTextValue}>
          {value}
        </span>
        <CopyInlineButton
          value={copyValue}
          label={`Copy ${label.toLowerCase()}`}
          copiedLabel={`${label} copied`}
          copiedChildren={null}
        >
          {null}
        </CopyInlineButton>
      </div>
    ) : (
      <span className={technical ? styles.settingsCodeValue : styles.settingsTextValue}>
        {value}
      </span>
    )}
  </div>
)

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.trim() ? error.message : fallback
}
