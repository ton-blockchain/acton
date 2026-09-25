import {useEffect, useMemo, useState} from "react"
import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
  InfoPopover,
  Skeleton,
} from "@acton/ui"

import type {TonClient} from "../api/client"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import {ExplorerAddressChip} from "./ExplorerAddressChip"
import {parseWalletV5Storage} from "./walletV5"
import {loadWalletV5PluginTypes, type WalletV5PluginType} from "./walletV5Plugins"
import styles from "./WalletV5PluginsTab.module.css"

interface WalletV5PluginsTabProps {
  readonly address: string
  readonly data: string | null | undefined
  readonly client: TonClient
  readonly onAddressClick: (address: string, event?: ExplorerNavigationClickEvent) => void
}

const NO_PLUGINS: readonly string[] = []
const TYPE_RETRY_DELAY_MS = 5000

export function WalletV5PluginsTab({
  address,
  data,
  client,
  onAddressClick,
}: WalletV5PluginsTabProps) {
  const metadataRegistry = useMetadataRegistry()
  const state = useMemo(() => {
    if (!data) return {status: "missing"} as const
    try {
      return {status: "success", storage: parseWalletV5Storage(data, address)} as const
    } catch {
      return {status: "invalid"} as const
    }
  }, [address, data])
  const signatureAuthEnabled = state.status === "success" && state.storage.isSignatureAllowed
  const extensions = state.status === "success" ? state.storage.extensions : NO_PLUGINS
  // With no extensions, W5 accepts a valid signed request and sets the stored flag to true.
  const signatureAuthAutoEnables =
    state.status === "success" && !signatureAuthEnabled && extensions.length === 0
  const typeRequest = useMemo(
    () => ({client, metadataRegistry, addresses: extensions}),
    [client, metadataRegistry, extensions],
  )
  const [resolvedTypes, setResolvedTypes] = useState<{
    readonly request: typeof typeRequest
    readonly types: ReadonlyMap<string, WalletV5PluginType>
  }>()
  const pluginTypes = resolvedTypes?.request === typeRequest ? resolvedTypes.types : undefined

  useEffect(() => {
    if (typeRequest.addresses.length === 0) return
    let active = true
    let timeoutId: ReturnType<typeof setTimeout> | undefined
    const resolved = new Map<string, WalletV5PluginType>()

    const load = async (addresses: readonly string[]) => {
      try {
        const types = await loadWalletV5PluginTypes({
          ...typeRequest,
          addresses,
          shouldContinue: () => active,
        })
        if (!active || !types) return
        for (const [address, type] of types) {
          if (type.status === "success") resolved.set(address, type)
        }
        setResolvedTypes({request: typeRequest, types: new Map(resolved)})
      } catch {
        // Keep unresolved cells loading until the next background attempt.
      }
      if (!active) return
      const unresolved = addresses.filter(address => !resolved.has(address))
      if (unresolved.length > 0) {
        timeoutId = globalThis.setTimeout(() => void load(unresolved), TYPE_RETRY_DELAY_MS)
      }
    }

    void load(typeRequest.addresses)
    return () => {
      active = false
      if (timeoutId !== undefined) globalThis.clearTimeout(timeoutId)
    }
  }, [typeRequest])

  return (
    <section aria-label="Plugins">
      {state.status === "success" ? (
        <>
          <div className={styles.toolbar}>
            <div className={styles.authorization}>
              <span className={styles.authorizationLabel}>
                Signature auth
                <InfoPopover
                  ariaLabel="About signature authorization"
                  placement="top"
                  openDelay={0}
                >
                  <strong>
                    Signature auth is {signatureAuthEnabled ? "enabled" : "disabled"}.
                  </strong>
                  <span>
                    {signatureAuthEnabled
                      ? "The wallet accepts transactions signed with its owner's key."
                      : signatureAuthAutoEnables
                        ? "No plugins are installed, so valid requests signed with the owner's key are still accepted. Processing a signed request automatically enables signature auth."
                        : "Transactions signed with the owner's key are rejected."}
                  </span>
                  {extensions.length > 0 && (
                    <span>
                      Installed plugins can authorize transactions independently of this setting.
                    </span>
                  )}
                </InfoPopover>
              </span>
              <span className={styles.authorizationValue}>
                <span
                  className={signatureAuthEnabled ? styles.enabledDot : styles.disabledDot}
                  aria-hidden="true"
                />
                {signatureAuthEnabled ? "Enabled" : "Disabled"}
              </span>
              {signatureAuthAutoEnables && (
                <span className={styles.authorizationHint}>(auto-enables on signed request)</span>
              )}
            </div>
          </div>
          <DataTable className={styles.flushTable} minWidth={0}>
            <DataTableTable aria-label="Installed plugins">
              <DataTableHead>
                <DataTableRow>
                  <DataTableHeaderCell>Address</DataTableHeaderCell>
                  <DataTableHeaderCell columnWidth="30%">Type</DataTableHeaderCell>
                </DataTableRow>
              </DataTableHead>
              <DataTableBody>
                {extensions.map(extension => (
                  <DataTableRow key={extension} hover>
                    <DataTableCell>
                      <ExplorerAddressChip
                        address={extension}
                        onAddressClick={onAddressClick}
                        resolveName={false}
                      />
                    </DataTableCell>
                    <DataTableCell tone="muted" className={styles.pluginType}>
                      <PluginType value={pluginTypes?.get(extension)} />
                    </DataTableCell>
                  </DataTableRow>
                ))}
                {extensions.length === 0 && (
                  <DataTableEmpty colSpan={2}>No plugins installed</DataTableEmpty>
                )}
              </DataTableBody>
            </DataTableTable>
          </DataTable>
        </>
      ) : (
        <div className={styles.unavailable}>
          <p className={styles.unavailableTitle}>Plugin data is unavailable</p>
          <p className={styles.description}>
            {state.status === "missing"
              ? "The account response did not include the wallet state."
              : "The wallet state could not be decoded."}
          </p>
        </div>
      )}
    </section>
  )
}

function PluginType({value}: {readonly value: WalletV5PluginType | undefined}) {
  return value?.status === "success" ? (
    value.labels.join(", ")
  ) : (
    <span aria-label="Loading plugin type">
      <Skeleton width="100%" height="0.875rem" />
    </span>
  )
}
