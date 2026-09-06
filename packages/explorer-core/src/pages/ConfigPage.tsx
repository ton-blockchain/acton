import {
  ByteSize,
  BooleanValue,
  ContentTabs,
  CopyInlineAction,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHeaderCell,
  DataTableHead,
  DataTableRow,
  DataTableTable,
  DateTime,
  Duration,
  formatNumberValue,
  GramAmount,
  InfoPopover,
  InlineActions,
  InlineButton,
  Input,
  NumberValue,
  ParsedValueView,
  Percentage,
  RawDataBlock,
  Skeleton,
  SkeletonText,
  TechnicalValue,
  TokenAmount,
  Tooltip,
} from "@acton/ui"
import {ArrowRight, ChevronDown, ExternalLink, Link2, Pencil, Search} from "lucide-react"
import {
  createContext,
  lazy,
  Suspense,
  useContext,
  useEffect,
  useMemo,
  useState,
  type FC,
  type MouseEvent,
  type ReactNode,
} from "react"
import {Link, useLocation, useParams} from "react-router"

import type {TonClient} from "../api/client"
import {
  TON_CONFIG_DOCS_URL,
  type BurningConfiguration,
  type BridgeConfiguration,
  type BridgeOracle,
  type ExtraCurrency,
  type FundamentalSmartContract,
  getConfigParameterMetadata,
  type GlobalVersionConfiguration,
  type NetworkConfig,
  type NetworkConfigParameter,
  type NetworkConfigValue,
  type PrecompiledContractConfiguration,
  type SuspendedAddressesConfiguration,
  type ValidatorConfiguration,
  type ValidatorRegistryConfiguration,
  type ValidatorSetConfiguration,
} from "../api/config"
import {getExtraCurrencyMetadata} from "../api/extraCurrency"
import {getPrecompiledContractMetadata} from "../api/precompiledContract"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {GlobalCapabilities} from "../components/GlobalCapabilities"
import type {TelegramWalletContractBytecode} from "../config/configParameterMinus123"
import tlbParameterIds from "../config/tlb.parameters.generated.json"
import {useConfigNavigation} from "../hooks/useConfigNavigation"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {useOpenExplorerPath} from "../hooks/useOpenExplorerPath"
import styles from "./ConfigPage.module.css"
import {getExternalAccountExplorerLink} from "./configExternalAccount"

const ConfigParameterTlb = lazy(() => import("../config/ConfigParameterTlb"))

interface ConfigPageProps {
  readonly client: TonClient
  /** Lets the host page own width and padding when embedding the configuration */
  readonly embedded?: boolean
  /** Keeps the index on the host application's preferred side of the parameter list */
  readonly navigationPosition?: "left" | "right"
  readonly showBreadcrumbs?: boolean
  readonly onError?: (message: string) => void
  readonly toolbar?: ReactNode
  readonly reloadKey?: number
  readonly onEdit?: (parameter: NetworkConfigParameter) => void
}

type ConfigLoadState =
  | {readonly status: "loading"}
  | {readonly status: "success"; readonly config: NetworkConfig}
  | {readonly status: "error"; readonly message: string}

const TON_CONFIG_DOC_PARAMETER_IDS: ReadonlySet<number> = new Set([
  0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 28,
  29, 30, 31, 32, 33, 34, 35, 36, 37, 39, 40, 43, 44, 45, 46, 71, 72, 73, 79, 81, 82,
])

function tonConfigDocsHref(id: number): string {
  if (id < 0) {
    return `${TON_CONFIG_DOCS_URL}#negative-parameters`
  }

  if (TON_CONFIG_DOC_PARAMETER_IDS.has(id)) {
    return `${TON_CONFIG_DOCS_URL}#${id}`
  }

  return TON_CONFIG_DOCS_URL
}

export const ConfigPage: FC<ConfigPageProps> = ({
  client,
  embedded = false,
  navigationPosition = "left",
  toolbar,
  reloadKey,
  onEdit,
  showBreadcrumbs = true,
  onError,
}) => {
  const {seqno: seqnoParam} = useParams<{seqno?: string}>()
  const seqno = parseConfigSeqno(seqnoParam)
  const routes = useExplorerRoutePaths()
  const [loadState, setLoadState] = useState<ConfigLoadState>({status: "loading"})
  const [query, setQuery] = useState("")

  useEffect(() => {
    let active = true

    const load = async () => {
      setLoadState({status: "loading"})
      if (seqnoParam !== undefined && seqno === undefined) {
        setLoadState({status: "error", message: "Invalid configuration block seqno"})
        onError?.("Invalid configuration block seqno")
        return
      }
      try {
        const config = await client.getNetworkConfig(seqno)
        if (active) setLoadState({status: "success", config})
      } catch (error) {
        if (active) {
          const message = error instanceof Error ? error.message : String(error)
          setLoadState({
            status: "error",
            message,
          })
          onError?.(message)
        }
      }
    }

    void load()
    return () => {
      active = false
    }
  }, [client, seqno, seqnoParam, reloadKey, onError])

  const config = loadState.status === "success" ? loadState.config : undefined
  const visibleParameters = useMemo(() => {
    if (!config) return []

    const normalizedQuery = query.trim().toLowerCase()
    if (!normalizedQuery) return config.parameters

    return config.parameters.filter(parameter =>
      [parameter.id.toString(), parameter.title, parameter.description]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery),
    )
  }, [config, query])

  return (
    <section className={`${styles.container} ${embedded ? styles.embedded : ""}`}>
      <header className={styles.header}>
        {showBreadcrumbs && (
          <ExplorerBreadcrumbs
            items={
              seqno === undefined
                ? [{label: "Config"}]
                : [{label: "Config", path: routes.configPath()}, {label: `Block #${seqno}`}]
            }
          />
        )}
        <Input
          aria-label="Filter configuration parameters"
          className={styles.filter}
          leadingIcon={<Search size={16} />}
          onChange={event => setQuery(event.currentTarget.value)}
          placeholder="Filter by number or name"
          size="md"
          value={query}
        />
      </header>

      {toolbar}

      {loadState.status === "loading" ? (
        <ConfigPageSkeleton navigationPosition={navigationPosition} />
      ) : loadState.status === "error" ? (
        !onError && (
          <section className={styles.error} role="alert">
            <h2>Network configuration is unavailable</h2>
            <p>{loadState.message}</p>
          </section>
        )
      ) : (
        <ConfigContent
          visibleParameters={visibleParameters}
          onEdit={onEdit}
          navigationPosition={navigationPosition}
        />
      )}
    </section>
  )
}

function scrollToConfigParameter(event: MouseEvent<HTMLAnchorElement>, id: number) {
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return

  event.preventDefault()
  const anchorId = `config-parameter-${id}`
  const hash = `#${anchorId}`

  if (globalThis.location.hash !== hash) {
    globalThis.history.pushState(globalThis.history.state, "", hash)
  }

  globalThis.document.getElementById(anchorId)?.scrollIntoView({
    behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth",
    block: "start",
  })
}

function parseConfigSeqno(value: string | undefined): number | undefined {
  if (value === undefined || !/^\d+$/.test(value)) return undefined

  const seqno = Number(value)
  return Number.isSafeInteger(seqno) ? seqno : undefined
}

function ConfigContent({
  visibleParameters,
  onEdit,
  navigationPosition,
}: {
  readonly onEdit?: (parameter: NetworkConfigParameter) => void
  readonly visibleParameters: readonly NetworkConfigParameter[]
  readonly navigationPosition: "left" | "right"
}) {
  const {activeId, contentRef, indexRef} = useConfigNavigation(visibleParameters)

  return (
    <>
      <div
        ref={contentRef}
        className={`${styles.configLayout} ${navigationPosition === "right" ? styles.navigationRight : ""}`}
      >
        <aside
          ref={indexRef}
          className={styles.indexPanel}
          aria-label="Configuration parameter index"
        >
          <nav className={styles.indexList}>
            {visibleParameters.map(parameter => (
              <a
                key={parameter.id}
                className={styles.indexLink}
                aria-current={activeId === parameter.id ? "location" : undefined}
                href={`#config-parameter-${parameter.id}`}
                onClick={event => scrollToConfigParameter(event, parameter.id)}
              >
                {parameter.id}. {parameter.title}
              </a>
            ))}
          </nav>
        </aside>

        <main className={styles.parameterList}>
          {visibleParameters.length === 0 ? (
            <section className={styles.empty}>
              <h2>No parameters found</h2>
              <p>Try a different number or search term</p>
            </section>
          ) : (
            visibleParameters.map(parameter => (
              <ConfigParameterCard
                key={parameter.id}
                parameter={parameter}
                actions={
                  onEdit && parameter.id === 0 ? (
                    <span className={styles.immutableLabel}>
                      Immutable
                      <InfoPopover
                        ariaLabel="Why the config address is immutable"
                        contentClassName={styles.infoContent}
                        placement="left"
                      >
                        <p>
                          Parameter 0 identifies the contract that stores the network configuration
                        </p>
                        <p>The contract cannot change its own address through a parameter update</p>
                      </InfoPopover>
                    </span>
                  ) : onEdit ? (
                    <InlineButton
                      className={styles.editAction}
                      leadingIcon={<Pencil className={styles.editIcon} />}
                      onClick={() => onEdit(parameter)}
                    >
                      Edit
                    </InlineButton>
                  ) : undefined
                }
              />
            ))
          )}
        </main>
      </div>
    </>
  )
}

/** Shared decoded parameter presentation, with optional application-owned actions. */
export function ConfigParameterCard({
  parameter,
  actions,
}: {
  readonly parameter: NetworkConfigParameter
  readonly actions?: ReactNode
}) {
  const hasValueTab =
    parameter.parsedValue !== undefined || parameter.contractBytecode !== undefined
  const hasCompactValue =
    parameter.address !== undefined ||
    parameter.contractBytecode !== undefined ||
    parameter.burningConfiguration !== undefined ||
    parameter.extraCurrencies !== undefined ||
    parameter.globalVersion !== undefined ||
    parameter.configurationValues !== undefined ||
    parameter.globalId !== undefined ||
    parameter.parameterIds !== undefined ||
    parameter.fundamentalSmartContracts !== undefined ||
    parameter.precompiledContracts !== undefined ||
    parameter.validatorRegistry !== undefined ||
    parameter.validatorSet !== undefined ||
    parameter.suspendedAddresses !== undefined ||
    parameter.bridgeConfiguration !== undefined
  const [activeTab, setActiveTab] = useState<"raw" | "value" | "tlb">(hasValueTab ? "value" : "raw")
  const tabs = [
    ...(hasValueTab ? [{label: "Value", value: "value" as const}] : []),
    {label: "Raw cell", value: "raw" as const},
    ...(tlbParameterIds.includes(parameter.id) ? [{label: "TL-B", value: "tlb" as const}] : []),
  ]

  return (
    <article id={`config-parameter-${parameter.id}`} className={styles.parameterCard}>
      <header className={styles.parameterHeader}>
        <ConfigParameterAnchor id={parameter.id} className={styles.parameterId} />
        <div className={styles.parameterTitleRow}>
          <h3 className={styles.parameterTitle}>{parameter.title}</h3>
          <InfoPopover
            ariaLabel={`About configuration parameter ${parameter.id}`}
            contentClassName={styles.infoContent}
          >
            <p>{parameter.description}</p>
            <a href={tonConfigDocsHref(parameter.id)} target="_blank" rel="noreferrer">
              Read the TON configuration reference
              <ExternalLink size={13} aria-hidden="true" />
            </a>
          </InfoPopover>
        </div>
        {actions && <div className={styles.parameterActions}>{actions}</div>}
        <p className={styles.parameterDescription}>{parameter.description}</p>
      </header>

      <ContentTabs
        ariaLabel={`Views for configuration parameter ${parameter.id}`}
        className={styles.parameterTabs}
        onValueChange={setActiveTab}
        panelClassName={`${styles.parameterTabPanel} ${
          activeTab !== "value" || hasCompactValue ? styles.compactParameterTabPanel : ""
        }`}
        tabs={tabs}
        value={activeTab}
      >
        {activeTab === "value" ? (
          <ConfigParameterValue parameter={parameter} />
        ) : activeTab === "tlb" ? (
          <Suspense
            fallback={
              <RawDataBlock
                value=""
                variant="embedded"
                loading
                loadingLabel="Loading TL-B schema"
              />
            }
          >
            <ConfigParameterTlb id={parameter.id} />
          </Suspense>
        ) : (
          <RawDataBlock
            className={styles.parameterBoc}
            copyLabel={`parameter ${parameter.id} cell`}
            maxHeight="16rem"
            value={parameter.rawHex}
            variant="embedded"
          />
        )}
      </ContentTabs>
    </article>
  )
}

function ConfigParameterAnchor({
  id,
  className,
  tooltip,
}: {
  readonly id: number
  readonly className?: string
  readonly tooltip?: ReactNode
}) {
  const anchor = (
    <a
      className={`${styles.parameterAnchor} ${className ?? ""}`}
      href={`#config-parameter-${id}`}
      aria-label={`Link to configuration parameter ${id}`}
      onClick={event => scrollToConfigParameter(event, id)}
    >
      <span className={styles.parameterAnchorNumber}>{id}</span>
      <Link2 className={styles.parameterAnchorIcon} size={16} aria-hidden="true" />
    </a>
  )

  return tooltip === undefined ? anchor : <Tooltip content={tooltip}>{anchor}</Tooltip>
}

/**
 * Renders a decoded config parameter and owns the comparison layout used by the editor.
 * Keeping comparison here guarantees that review uses the same domain-specific values
 * as the read-only config page, including address chips, GRAM amounts, and tables.
 */
export function ConfigParameterValue({
  parameter,
  comparison,
}: {
  readonly parameter: NetworkConfigParameter
  readonly comparison?: {readonly before?: NetworkConfigParameter}
}) {
  if (comparison !== undefined) {
    return <ConfigParameterValueDiff before={comparison.before} after={parameter} />
  }

  return <ConfigParameterValueContent parameter={parameter} />
}

function ConfigParameterValueDiff({
  before,
  after,
}: {
  readonly before?: NetworkConfigParameter
  readonly after: NetworkConfigParameter
}) {
  if (before === undefined) {
    return (
      <div className={styles.configValueDiffAdded}>
        <ConfigParameterValueSnapshot label="New value" tone="after" parameter={after} />
      </div>
    )
  }

  const changedItemIds = findChangedGridItemIds(before, after)

  return (
    <div className={styles.configValueDiff}>
      <ConfigParameterValueSnapshot
        changedItemIds={changedItemIds}
        label="Current value"
        tone="before"
        parameter={before}
      />

      <ArrowRight className={styles.configValueDiffArrow} aria-hidden="true" />

      <ConfigParameterValueSnapshot
        changedItemIds={changedItemIds}
        label="New value"
        tone="after"
        parameter={after}
      />
    </div>
  )
}

function ConfigParameterValueSnapshot({
  changedItemIds,
  label,
  tone,
  parameter,
}: {
  readonly changedItemIds?: ReadonlySet<string>
  readonly label: string
  readonly tone: "before" | "after"
  readonly parameter: NetworkConfigParameter
}) {
  const className = tone === "before" ? styles.configValueDiffBefore : styles.configValueDiffAfter

  return (
    <section className={`${styles.configValueDiffSnapshot} ${className}`}>
      <h3 className={styles.configValueDiffLabel}>{label}</h3>

      <ConfigValueDiffContext.Provider value={{changedItemIds, tone}}>
        <div className={styles.configValueDiffContent}>
          {hasDecodedConfigValue(parameter) ? (
            <ConfigParameterValueContent parameter={parameter} />
          ) : (
            <RawDataBlock
              className={styles.configValueDiffRaw}
              copyLabel={`${label.toLowerCase()} BoC`}
              maxHeight="14rem"
              value={parameter.rawHex}
              variant="embedded"
            />
          )}
        </div>
      </ConfigValueDiffContext.Provider>
    </section>
  )
}

function hasDecodedConfigValue(parameter: NetworkConfigParameter): boolean {
  return parameter.parsedValue !== undefined || parameter.contractBytecode !== undefined
}

interface ConfigValueDiffState {
  readonly changedItemIds?: ReadonlySet<string>
  readonly tone: "before" | "after"
}

const ConfigValueDiffContext = createContext<ConfigValueDiffState | undefined>(undefined)

function findChangedGridItemIds(
  before: NetworkConfigParameter,
  after: NetworkConfigParameter,
): ReadonlySet<string> {
  const changed = new Set<string>()

  if (before.contractBytecode || after.contractBytecode) {
    markChanged(
      changed,
      "contract-bytecode-hash",
      before.contractBytecode?.bytecodeHash,
      after.contractBytecode?.bytecodeHash,
    )
    markChanged(
      changed,
      "contract-bytecode-revision",
      before.contractBytecode?.revision,
      after.contractBytecode?.revision,
    )
    markChanged(
      changed,
      "contract-bytecode-repository",
      before.contractBytecode?.repositoryUrl,
      after.contractBytecode?.repositoryUrl,
    )
  } else if (before.burningConfiguration || after.burningConfiguration) {
    markChanged(
      changed,
      "fee-burn-num",
      before.burningConfiguration?.feeBurnNum,
      after.burningConfiguration?.feeBurnNum,
    )
    markChanged(
      changed,
      "fee-burn-denom",
      before.burningConfiguration?.feeBurnDenom,
      after.burningConfiguration?.feeBurnDenom,
    )
    markChanged(
      changed,
      "blackhole-address",
      before.burningConfiguration?.blackholeAddress,
      after.burningConfiguration?.blackholeAddress,
    )
  } else if (before.extraCurrencies || after.extraCurrencies) {
    collectChangedCurrencies(changed, before.extraCurrencies ?? [], after.extraCurrencies ?? [])
  } else if (before.globalVersion || after.globalVersion) {
    markChanged(changed, "version", before.globalVersion?.version, after.globalVersion?.version)
    markChanged(
      changed,
      "capabilities",
      before.globalVersion?.capabilities,
      after.globalVersion?.capabilities,
    )
  } else if (before.configurationValues || after.configurationValues) {
    collectChangedConfigValues(
      changed,
      before.configurationValues ?? [],
      after.configurationValues ?? [],
      "configuration-value",
    )
  } else if (before.parameterIds || after.parameterIds) {
    collectChangedScalarKeys(
      changed,
      before.parameterIds ?? [],
      after.parameterIds ?? [],
      id => `parameter-id-${id}`,
    )
  } else if (before.fundamentalSmartContracts || after.fundamentalSmartContracts) {
    collectChangedRecords(
      changed,
      before.fundamentalSmartContracts ?? [],
      after.fundamentalSmartContracts ?? [],
      contract => contract.address,
      contract => contract.codeHash,
      address => `fundamental-contract-${address}`,
    )
  } else if (before.precompiledContracts || after.precompiledContracts) {
    collectChangedRecords(
      changed,
      before.precompiledContracts ?? [],
      after.precompiledContracts ?? [],
      contract => contract.codeHash,
      contract => `${contract.index}:${contract.gasUsage}`,
      codeHash => `precompiled-contract-${codeHash}`,
    )
  } else if (before.validatorRegistry || after.validatorRegistry) {
    markChanged(
      changed,
      "validator-registry-address",
      before.validatorRegistry?.contractAddress,
      after.validatorRegistry?.contractAddress,
    )
    markChanged(
      changed,
      "max-collators-per-validator",
      before.validatorRegistry?.maxCollatorsPerValidator,
      after.validatorRegistry?.maxCollatorsPerValidator,
    )
    markChanged(
      changed,
      "validator-registry-new-code-hash",
      before.validatorRegistry?.newCodeHash,
      after.validatorRegistry?.newCodeHash,
    )
  } else if (before.validatorSet || after.validatorSet) {
    markChanged(
      changed,
      "utime-since",
      before.validatorSet?.utimeSince,
      after.validatorSet?.utimeSince,
    )
    markChanged(
      changed,
      "utime-until",
      before.validatorSet?.utimeUntil,
      after.validatorSet?.utimeUntil,
    )
    markChanged(changed, "total", before.validatorSet?.total, after.validatorSet?.total)
    markChanged(changed, "main", before.validatorSet?.main, after.validatorSet?.main)
    markChanged(
      changed,
      "total-weight",
      before.validatorSet?.totalWeight,
      after.validatorSet?.totalWeight,
    )
    collectChangedRecords(
      changed,
      before.validatorSet?.validators ?? [],
      after.validatorSet?.validators ?? [],
      validator => validator.index,
      validator =>
        `${validator.publicKey}:${validator.adnlAddress ?? ""}:${validator.weight}:${
          validator.index < (before.validatorSet?.main ?? 0)
        }`,
      index => `validator-${index}`,
      validator =>
        `${validator.publicKey}:${validator.adnlAddress ?? ""}:${validator.weight}:${
          validator.index < (after.validatorSet?.main ?? 0)
        }`,
    )
  } else if (before.suspendedAddresses || after.suspendedAddresses) {
    markChanged(
      changed,
      "suspended-until",
      before.suspendedAddresses?.suspendedUntil,
      after.suspendedAddresses?.suspendedUntil,
    )
    markChanged(
      changed,
      "suspended-addresses-page",
      before.suspendedAddresses?.addresses.join(","),
      after.suspendedAddresses?.addresses.join(","),
    )
    collectChangedScalarKeys(
      changed,
      before.suspendedAddresses?.addresses ?? [],
      after.suspendedAddresses?.addresses ?? [],
      address => `suspended-address-${address}`,
    )
  } else if (before.bridgeConfiguration || after.bridgeConfiguration) {
    collectChangedBridgeFields(changed, before.bridgeConfiguration, after.bridgeConfiguration)
  }

  return changed
}

function collectChangedScalarKeys<T extends bigint | number | string>(
  changed: Set<string>,
  before: readonly T[],
  after: readonly T[],
  itemId: (value: T) => string,
) {
  const beforeValues = new Set(before)
  const afterValues = new Set(after)

  for (const value of new Set([...beforeValues, ...afterValues])) {
    if (!beforeValues.has(value) || !afterValues.has(value)) changed.add(itemId(value))
  }
}

function collectChangedRecords<T, K>(
  changed: Set<string>,
  before: readonly T[],
  after: readonly T[],
  key: (value: T) => K,
  beforeFingerprint: (value: T) => string,
  itemId: (key: K) => string,
  afterFingerprint: (value: T) => string = beforeFingerprint,
) {
  const beforeByKey = new Map(before.map(value => [key(value), value]))
  const afterByKey = new Map(after.map(value => [key(value), value]))

  for (const recordKey of new Set([...beforeByKey.keys(), ...afterByKey.keys()])) {
    const previous = beforeByKey.get(recordKey)
    const next = afterByKey.get(recordKey)

    if (!previous || !next || beforeFingerprint(previous) !== afterFingerprint(next)) {
      changed.add(itemId(recordKey))
    }
  }
}

function markChanged(
  changed: Set<string>,
  id: string,
  before: bigint | boolean | number | string | undefined,
  after: bigint | boolean | number | string | undefined,
) {
  if (before !== after) changed.add(id)
}

function collectChangedCurrencies(
  changed: Set<string>,
  before: readonly ExtraCurrency[],
  after: readonly ExtraCurrency[],
) {
  const beforeById = new Map(before.map(currency => [currency.id, currency]))
  const afterById = new Map(after.map(currency => [currency.id, currency]))
  const ids = new Set([...beforeById.keys(), ...afterById.keys()])

  for (const id of ids) {
    const previous = beforeById.get(id)
    const next = afterById.get(id)

    if (!previous || !next) changed.add(`currency-${id}`)
    markChanged(changed, `currency-${id}-amount`, previous?.amount, next?.amount)
  }
}

function collectChangedConfigValues(
  changed: Set<string>,
  before: readonly NetworkConfigValue[],
  after: readonly NetworkConfigValue[],
  idPrefix: string,
) {
  const length = Math.max(before.length, after.length)

  for (let index = 0; index < length; index += 1) {
    const id = `${idPrefix}-${index}`
    const previous = before[index]
    const next = after[index]

    if (!previous || !next || previous.label !== next.label || previous.format !== next.format) {
      changed.add(id)
      continue
    }

    if (previous.children || next.children) {
      collectChangedConfigValues(changed, previous.children ?? [], next.children ?? [], id)
      continue
    }

    markChanged(changed, id, previous.value, next.value)
  }
}

function collectChangedBridgeFields(
  changed: Set<string>,
  before: BridgeConfiguration | undefined,
  after: BridgeConfiguration | undefined,
) {
  markChanged(changed, "bridge-address", before?.bridgeAddress, after?.bridgeAddress)
  markChanged(changed, "oracle-address", before?.oracleAddress, after?.oracleAddress)
  markChanged(
    changed,
    "external-chain-address",
    before?.externalChainAddress,
    after?.externalChainAddress,
  )
  markChanged(changed, "state-flags", before?.stateFlags, after?.stateFlags)
  markChanged(changed, "burn-bridge-fee", before?.burnBridgeFee, after?.burnBridgeFee)

  collectChangedConfigValues(changed, before?.prices ?? [], after?.prices ?? [], "bridge-price")
  collectChangedRecords(
    changed,
    before?.oracles ?? [],
    after?.oracles ?? [],
    oracle => oracle.address,
    oracle => oracle.externalAddress,
    address => `bridge-oracle-${address}`,
  )
}

function ConfigParameterValueContent({parameter}: {readonly parameter: NetworkConfigParameter}) {
  if (parameter.address !== undefined) {
    return (
      <div className={styles.parsedValue}>
        <ConfigAddressValue address={parameter.address} />
      </div>
    )
  }

  if (parameter.contractBytecode !== undefined) {
    return <ContractBytecodeValue configuration={parameter.contractBytecode} />
  }

  if (parameter.burningConfiguration !== undefined) {
    return <BurningConfigurationValue configuration={parameter.burningConfiguration} />
  }

  if (parameter.extraCurrencies !== undefined) {
    return <ExtraCurrenciesValue currencies={parameter.extraCurrencies} />
  }

  if (parameter.globalVersion !== undefined) {
    return <GlobalVersionValue configuration={parameter.globalVersion} />
  }

  if (parameter.configurationValues !== undefined) {
    return <ConfigurationValuesValue values={parameter.configurationValues} />
  }

  if (parameter.globalId !== undefined) {
    return (
      <div className={styles.parsedValue}>
        <NumberValue className={styles.globalIdValue} value={parameter.globalId} />
      </div>
    )
  }

  if (parameter.parameterIds !== undefined) {
    return <ConfigParameterIdList ids={parameter.parameterIds} />
  }

  if (parameter.fundamentalSmartContracts !== undefined) {
    return (
      <div className={styles.fundamentalValue}>
        <FundamentalSmartContractsTable contracts={parameter.fundamentalSmartContracts} />
      </div>
    )
  }

  if (parameter.precompiledContracts !== undefined) {
    return <PrecompiledContractsTable contracts={parameter.precompiledContracts} />
  }

  if (parameter.validatorRegistry !== undefined) {
    return <ValidatorRegistryValue configuration={parameter.validatorRegistry} />
  }

  if (parameter.validatorSet !== undefined) {
    return <ValidatorSetValue configuration={parameter.validatorSet} />
  }

  if (parameter.suspendedAddresses !== undefined) {
    return <SuspendedAddressesValue configuration={parameter.suspendedAddresses} />
  }

  if (parameter.bridgeConfiguration !== undefined) {
    return <BridgeConfigurationValue configuration={parameter.bridgeConfiguration} />
  }

  if (parameter.parsedValue) {
    return (
      <div className={styles.parsedValue}>
        <ParsedValueView value={parameter.parsedValue} />
      </div>
    )
  }

  return (
    <div className={styles.parseNotice}>
      <Skeleton shape="circle" width="6px" height="6px" animated={false} />
      <span>
        {parameter.parseError
          ? "Typed decoding is unavailable for this cell"
          : "This parameter is available as raw cell data"}
      </span>
    </div>
  )
}

function ContractBytecodeValue({
  configuration,
}: {
  readonly configuration: TelegramWalletContractBytecode
}) {
  return (
    <ConfigValueGrid
      items={[
        {
          id: "contract-bytecode-hash",
          label: "code_hash",
          value: (
            <TechnicalValue
              copyLabel="contract bytecode hash"
              shorten={false}
              value={configuration.bytecodeHash}
            />
          ),
          wide: true,
        },
        {
          id: "contract-bytecode-revision",
          label: "revision",
          value: configuration.revision ?? "Unknown",
        },
        {
          id: "contract-bytecode-repository",
          label: "GitHub repository",
          value:
            configuration.repositoryUrl === undefined ? (
              "Unknown"
            ) : (
              <a
                className={styles.externalAccountLink}
                href={configuration.repositoryUrl}
                target="_blank"
                rel="noreferrer"
              >
                ton-blockchain/tg-wallet-contract
                <ExternalLink size={13} aria-hidden="true" />
              </a>
            ),
        },
      ]}
    />
  )
}

function ConfigAddressValue({
  address,
  shorten = false,
}: {
  readonly address: string
  readonly shorten?: boolean
}) {
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()

  return (
    <ExplorerAddressChip
      address={address}
      onAddressClick={(value, event) => openPath(routes.addressPath(value), event)}
      resolveName={false}
      shorten={shorten}
    />
  )
}

function ConfigTechnicalNumberValue({
  value,
  copyLabel,
}: {
  readonly value: bigint | number
  readonly copyLabel: string
}) {
  return (
    <TechnicalValue
      copyLabel={copyLabel}
      displayValue={formatNumberValue(value)}
      shorten={false}
      value={String(value)}
    />
  )
}

function BurningConfigurationValue({
  configuration,
}: {
  readonly configuration: BurningConfiguration
}) {
  return (
    <ConfigValueGrid
      items={[
        {
          id: "fee-burn-num",
          label: "Fee burn num",
          value: <NumberValue value={configuration.feeBurnNum} />,
        },
        {
          id: "fee-burn-denom",
          label: "Fee burn denom",
          value: <NumberValue value={configuration.feeBurnDenom} />,
        },
        {
          id: "blackhole-address",
          label: "Blackhole address",
          value: configuration.blackholeAddress ? (
            <ConfigAddressValue address={configuration.blackholeAddress} />
          ) : (
            "Not configured"
          ),
          wide: true,
        },
      ]}
    />
  )
}

function ExtraCurrenciesValue({currencies}: {readonly currencies: readonly ExtraCurrency[]}) {
  const routes = useExplorerRoutePaths()

  if (currencies.length === 0) {
    return <div className={styles.parsedValue}>No extra currencies configured</div>
  }

  return (
    <ConfigValueGrid
      items={currencies.map(currency => {
        const metadata = getExtraCurrencyMetadata(currency.id)
        const originSource = metadata.origin?.source
        const originHref =
          originSource?.kind === "transaction"
            ? routes.transactionPath(originSource.hash)
            : originSource?.url

        return {
          id: `currency-${currency.id}`,
          label:
            metadata.origin === undefined ? (
              metadata.symbol
            ) : (
              <span className={styles.extraCurrencyLabel}>
                {metadata.symbol}
                <InfoPopover
                  ariaLabel={`About extra currency ${metadata.symbol}`}
                  contentClassName={styles.infoContent}
                >
                  <p>{metadata.origin.label}</p>
                  <a href={originHref} target="_blank" rel="noreferrer">
                    {metadata.origin.linkLabel}
                    <ExternalLink size={13} aria-hidden="true" />
                  </a>
                </InfoPopover>
              </span>
            ),
          wide: true,
          children: [
            {
              id: `currency-${currency.id}-id`,
              label: "Currency ID",
              value: <NumberValue value={currency.id} />,
            },
            {
              id: `currency-${currency.id}-amount`,
              label: "Total supply",
              value: (
                <TokenAmount
                  decimals={metadata.decimals}
                  rawUnitsLabel="Raw amount"
                  symbol={metadata.symbol}
                  useGrouping
                  value={currency.amount}
                />
              ),
            },
          ],
        }
      })}
    />
  )
}

function GlobalVersionValue({configuration}: {readonly configuration: GlobalVersionConfiguration}) {
  return (
    <ConfigValueGrid
      items={[
        {
          id: "version",
          label: "Version",
          value: <NumberValue value={configuration.version} />,
        },
        {
          id: "capabilities",
          label: "Capabilities",
          value: <GlobalCapabilities value={configuration.capabilities} />,
        },
      ]}
    />
  )
}

function ValidatorRegistryValue({
  configuration,
}: {
  readonly configuration: ValidatorRegistryConfiguration
}) {
  return (
    <ConfigValueGrid
      items={[
        {
          id: "validator-registry-address",
          label: "Registry contract",
          value: <ConfigAddressValue address={configuration.contractAddress} />,
          wide: true,
        },
        {
          id: "max-collators-per-validator",
          label: "Max collators per validator",
          value: <NumberValue value={configuration.maxCollatorsPerValidator} />,
        },
        {
          id: "validator-registry-new-code-hash",
          label: "New code hash",
          value:
            configuration.newCodeHash === undefined ? (
              "Not configured"
            ) : (
              <TechnicalValue
                copyLabel="validator registry new code hash"
                shorten={false}
                value={configuration.newCodeHash}
              />
            ),
          wide: true,
        },
      ]}
    />
  )
}

function ConfigurationValuesValue({values}: {readonly values: readonly NetworkConfigValue[]}) {
  return (
    <ConfigValueGrid
      items={values.map((item, index) =>
        toConfigValueGridItem(item, `configuration-value-${index}`),
      )}
    />
  )
}

function ValidatorSetValue({configuration}: {readonly configuration: ValidatorSetConfiguration}) {
  return (
    <div className={styles.validatorSetValue}>
      <ConfigValueGrid
        items={[
          {
            id: "utime-since",
            label: "Utime since",
            value: <DateTime display="date-time" unit="seconds" value={configuration.utimeSince} />,
          },
          {
            id: "utime-until",
            label: "Utime until",
            value: <DateTime display="date-time" unit="seconds" value={configuration.utimeUntil} />,
          },
          {
            id: "total",
            label: "Total validators",
            value: <NumberValue value={configuration.total} />,
          },
          {
            id: "main",
            label: "Masterchain validators",
            value: <NumberValue value={configuration.main} />,
          },
          ...(configuration.totalWeight === undefined
            ? []
            : [
                {
                  id: "total-weight",
                  label: "Total weight",
                  value: <NumberValue value={configuration.totalWeight} />,
                },
              ]),
        ]}
      />
      <ValidatorList
        mainValidators={configuration.main}
        validators={configuration.validators}
        totalWeight={configuration.totalWeight}
      />
    </div>
  )
}

const VALIDATOR_PREVIEW_COUNT = 7

function ValidatorList({
  mainValidators,
  validators,
  totalWeight,
}: {
  readonly mainValidators: number
  readonly validators: readonly ValidatorConfiguration[]
  readonly totalWeight?: bigint
}) {
  const [expanded, setExpanded] = useState(false)
  const diff = useContext(ConfigValueDiffContext)
  const effectiveTotalWeight =
    totalWeight ?? validators.reduce((sum, validator) => sum + validator.weight, 0n)
  const hasMore = validators.length > VALIDATOR_PREVIEW_COUNT
  const visibleValidators = expanded
    ? validators
    : validators.slice(0, VALIDATOR_PREVIEW_COUNT + (hasMore ? 1 : 0))

  return (
    <div className={styles.validatorList}>
      <DataTable minWidth="50rem" variant="nested">
        <DataTableTable aria-label="Validators">
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell columnWidth="3.5rem">#</DataTableHeaderCell>
              <DataTableHeaderCell>Public key</DataTableHeaderCell>
              <DataTableHeaderCell>ADNL</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="8rem">Masterchain</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="13rem">Weight share</DataTableHeaderCell>
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {visibleValidators.length === 0 ? (
              <DataTableEmpty colSpan={5}>No validators configured</DataTableEmpty>
            ) : (
              visibleValidators.map(validator => (
                <DataTableRow
                  key={validator.index}
                  className={changedValueItemClass(diff, `validator-${validator.index}`)}
                >
                  <DataTableCell className={styles.validatorIndex} tone="muted">
                    {validator.index + 1}
                  </DataTableCell>
                  <DataTableCell className={styles.validatorHash} truncate>
                    <TechnicalValue
                      copyLabel="validator public key"
                      endLength={10}
                      startLength={10}
                      value={validator.publicKey}
                    />
                  </DataTableCell>
                  <DataTableCell className={styles.validatorHash} truncate>
                    <TechnicalValue
                      copyLabel="validator ADNL address"
                      endLength={10}
                      fallback="—"
                      startLength={10}
                      value={validator.adnlAddress}
                    />
                  </DataTableCell>
                  <DataTableCell>
                    <BooleanValue value={validator.index < mainValidators} />
                  </DataTableCell>
                  <DataTableCell className={styles.validatorWeight}>
                    <Tooltip
                      content={`${formatNumberValue(validator.weight)} of ${formatNumberValue(effectiveTotalWeight)}`}
                    >
                      <span className={styles.validatorWeightShare}>
                        <Percentage
                          maximumFractionDigits={3}
                          minimumFractionDigits={2}
                          total={Number(effectiveTotalWeight)}
                          value={validator.weight}
                        />
                      </span>
                    </Tooltip>
                  </DataTableCell>
                </DataTableRow>
              ))
            )}
          </DataTableBody>
        </DataTableTable>
      </DataTable>
      {hasMore && !expanded ? (
        <div className={styles.validatorListFade} aria-hidden="true" />
      ) : null}
      {hasMore ? (
        <button
          type="button"
          className={`${styles.validatorShowMore} ${expanded ? styles.validatorShowLess : ""}`}
          aria-expanded={expanded}
          onClick={() => setExpanded(value => !value)}
        >
          {expanded ? "Show less" : "Show more"}
          <ChevronDown
            className={expanded ? styles.validatorShowMoreExpanded : undefined}
            size={18}
            aria-hidden="true"
          />
        </button>
      ) : null}
    </div>
  )
}

function SuspendedAddressesValue({
  configuration,
}: {
  readonly configuration: SuspendedAddressesConfiguration
}) {
  const [expanded, setExpanded] = useState(false)
  const diff = useContext(ConfigValueDiffContext)
  const routes = useExplorerRoutePaths()
  const {search} = useLocation()
  const hasMore = configuration.addresses.length > VALIDATOR_PREVIEW_COUNT
  const visibleAddresses = expanded
    ? configuration.addresses
    : configuration.addresses.slice(0, VALIDATOR_PREVIEW_COUNT + (hasMore ? 1 : 0))

  return (
    <div className={styles.validatorSetValue}>
      <ConfigValueGrid
        items={[
          {
            id: "suspended-until",
            label: "Suspended until",
            value: (
              <DateTime display="compact" unit="seconds" value={configuration.suspendedUntil} />
            ),
          },
          {
            id: "suspended-addresses-page",
            label: "Suspended addresses page",
            value: (
              <Link
                className={styles.configValueLink}
                to={{pathname: routes.suspendedAddressesPath, search}}
              >
                Open overview
              </Link>
            ),
          },
        ]}
      />
      <div className={styles.validatorList}>
        <DataTable minWidth="42rem" variant="nested">
          <DataTableTable aria-label="Suspended addresses">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="3.5rem">#</DataTableHeaderCell>
                <DataTableHeaderCell>Address</DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {visibleAddresses.length === 0 ? (
                <DataTableEmpty colSpan={2}>No suspended addresses</DataTableEmpty>
              ) : (
                visibleAddresses.map((address, index) => (
                  <DataTableRow
                    key={address}
                    className={changedValueItemClass(diff, `suspended-address-${address}`)}
                  >
                    <DataTableCell className={styles.validatorIndex} tone="muted">
                      {index + 1}
                    </DataTableCell>
                    <DataTableCell className={styles.validatorHash} truncate>
                      <ConfigAddressValue address={address} />
                    </DataTableCell>
                  </DataTableRow>
                ))
              )}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
        {hasMore && !expanded ? (
          <div className={styles.validatorListFade} aria-hidden="true" />
        ) : null}
        {hasMore ? (
          <button
            type="button"
            className={`${styles.validatorShowMore} ${expanded ? styles.validatorShowLess : ""}`}
            aria-expanded={expanded}
            onClick={() => setExpanded(value => !value)}
          >
            {expanded ? "Show less" : "Show more"}
            <ChevronDown
              className={expanded ? styles.validatorShowMoreExpanded : undefined}
              size={18}
              aria-hidden="true"
            />
          </button>
        ) : null}
      </div>
    </div>
  )
}

function BridgeConfigurationValue({configuration}: {readonly configuration: BridgeConfiguration}) {
  const oracleLabel =
    configuration.kind === "oracle" ? "Oracle multisig address" : "Oracles address"

  return (
    <div className={styles.validatorSetValue}>
      <ConfigValueGrid
        items={[
          {
            id: "bridge-address",
            label: "Bridge address",
            value: <ConfigAddressValue address={configuration.bridgeAddress} shorten />,
          },
          {
            id: "oracle-address",
            label: oracleLabel,
            value: <ConfigAddressValue address={configuration.oracleAddress} shorten />,
          },
          ...(configuration.externalChainAddress === undefined
            ? []
            : [
                {
                  id: "external-chain-address",
                  label: `${configuration.externalChain} bridge address`,
                  value: (
                    <ExternalAccountValue
                      address={configuration.externalChainAddress}
                      copyLabel="external chain address"
                      externalChain={configuration.externalChain}
                      shorten={false}
                    />
                  ),
                  wide: true,
                },
              ]),
          ...(configuration.stateFlags === undefined
            ? []
            : [
                {
                  id: "state-flags",
                  label: "State flags",
                  value: <NumberValue value={configuration.stateFlags} />,
                },
              ]),
          ...(configuration.burnBridgeFee === undefined
            ? []
            : [
                {
                  id: "burn-bridge-fee",
                  label: "Burn bridge fee",
                  value: <GramAmount value={configuration.burnBridgeFee} useGrouping />,
                },
              ]),
        ]}
      />
      {configuration.prices === undefined ? null : (
        <ConfigValueGrid
          items={[
            {
              id: "prices",
              label: "Prices",
              children: configuration.prices.map((item, index) =>
                toConfigValueGridItem(item, `bridge-price-${index}`),
              ),
            },
          ]}
        />
      )}
      <BridgeOracleTable
        externalChain={configuration.externalChain}
        oracles={configuration.oracles}
      />
    </div>
  )
}

function BridgeOracleTable({
  externalChain,
  oracles,
}: {
  readonly externalChain: BridgeConfiguration["externalChain"]
  readonly oracles: readonly BridgeOracle[]
}) {
  const diff = useContext(ConfigValueDiffContext)

  return (
    <DataTable
      className={styles.tableInset}
      meta={`${oracles.length} items`}
      minWidth="42rem"
      title="Oracles"
      variant="nested"
    >
      <DataTableTable aria-label="Bridge oracles">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell>TON address</DataTableHeaderCell>
            <DataTableHeaderCell>{externalChain} address</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {oracles.length === 0 ? (
            <DataTableEmpty colSpan={2}>No oracles configured</DataTableEmpty>
          ) : (
            oracles.map(oracle => (
              <DataTableRow
                key={oracle.address}
                className={changedValueItemClass(diff, `bridge-oracle-${oracle.address}`)}
              >
                <DataTableCell className={styles.validatorHash} truncate>
                  <ConfigAddressValue address={oracle.address} shorten />
                </DataTableCell>
                <DataTableCell className={styles.validatorHash} truncate>
                  <ExternalAccountValue
                    address={oracle.externalAddress}
                    copyLabel={`${externalChain} oracle address`}
                    externalChain={externalChain}
                  />
                </DataTableCell>
              </DataTableRow>
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function ExternalAccountValue({
  address,
  copyLabel,
  externalChain,
  shorten = true,
}: {
  readonly address: string
  readonly copyLabel: string
  readonly externalChain: BridgeConfiguration["externalChain"]
  readonly shorten?: boolean
}) {
  const {network} = useNetworkInfo()
  const explorer = getExternalAccountExplorerLink(externalChain, network.id === "testnet", address)
  if (explorer === undefined) {
    return (
      <TechnicalValue
        copyLabel={copyLabel}
        endLength={8}
        shorten={shorten}
        startLength={8}
        value={address}
      />
    )
  }

  return (
    <InlineActions
      actions={
        <CopyInlineAction
          copiedLabel={`${copyLabel} copied`}
          label={`Copy ${copyLabel}`}
          size="compact"
          value={address}
        />
      }
    >
      <a
        aria-label={`Open ${externalChain} account ${address} in ${explorer.name}`}
        className={styles.externalAccountLink}
        href={explorer.href}
        target="_blank"
        rel="noreferrer"
      >
        <TechnicalValue
          copyable={false}
          endLength={8}
          shorten={shorten}
          startLength={8}
          value={address}
        />
        <ExternalLink size={12} aria-hidden="true" />
      </a>
    </InlineActions>
  )
}

function toConfigValueGridItem(item: NetworkConfigValue, id: string): ConfigValueGridItem {
  if (item.children !== undefined) {
    return {
      id,
      label: item.label,
      children: item.children.map((child, index) => toConfigValueGridItem(child, `${id}-${index}`)),
    }
  }

  return {
    id,
    label: item.label,
    value: renderConfigValue(item),
  }
}

function renderConfigValue(item: NetworkConfigValue): ReactNode {
  if (item.value === undefined) return "Not available"
  if (typeof item.value === "boolean") return item.value ? "Enabled" : "Disabled"

  if (item.format === "bytes") {
    return <ByteSize value={typeof item.value === "number" ? item.value : undefined} />
  }
  if (item.format === "date") {
    if (item.value === 0) return "Initial"

    return (
      <DateTime
        display="date"
        unit="seconds"
        value={typeof item.value === "number" ? item.value : undefined}
      />
    )
  }
  if (item.format === "duration") {
    return (
      <Duration
        display="readable"
        value={typeof item.value === "number" ? item.value : undefined}
      />
    )
  }
  if (item.format === "duration-ms") {
    return (
      <Duration
        display="readable"
        unit="milliseconds"
        value={typeof item.value === "number" ? item.value : undefined}
      />
    )
  }
  if (item.format === "gram") {
    return <GramAmount value={item.value} useGrouping />
  }
  if (item.format === "gram-per-65536") {
    return <GramAmount value={scaleForwardPrice(item.value)} useGrouping />
  }

  return <NumberValue value={item.value} />
}

function scaleForwardPrice(value: bigint | number): bigint | number {
  return typeof value === "bigint" ? value / 65_536n : Math.trunc(value / 65_536)
}

function ConfigParameterIdList({ids}: {readonly ids: readonly number[]}) {
  const diff = useContext(ConfigValueDiffContext)

  return (
    <ul className={styles.configParameterIdList} aria-label="Configuration parameter IDs">
      {ids.length === 0 ? (
        <li className={styles.configParameterIdEmpty}>No parameters configured</li>
      ) : (
        ids.map(id => {
          const metadata = getConfigParameterMetadata(id)

          return (
            <li key={id} className={styles.configParameterIdItem}>
              <ConfigParameterAnchor
                id={id}
                className={`${styles.configParameterIdAnchor} ${changedValueItemClass(
                  diff,
                  `parameter-id-${id}`,
                )}`}
                tooltip={
                  <span className={styles.configParameterIdTooltip}>
                    <strong>{metadata.title}</strong>
                    <span>{metadata.description}</span>
                  </span>
                }
              />
            </li>
          )
        })
      )}
    </ul>
  )
}

interface ConfigValueGridItem {
  readonly label: ReactNode
  readonly value?: ReactNode
  readonly children?: readonly ConfigValueGridItem[]
  readonly wide?: boolean
  readonly id: string
}

function ConfigValueGrid({
  items,
  nested = false,
}: {
  readonly items: readonly ConfigValueGridItem[]
  readonly nested?: boolean
}) {
  const diff = useContext(ConfigValueDiffContext)

  return (
    <div className={`${styles.configValueGrid} ${nested ? styles.configValueGridNested : ""}`}>
      {items.map(item =>
        item.children === undefined ? (
          <div
            key={item.id}
            className={`${styles.configValueGridItem} ${
              item.wide ? styles.configValueGridItemWide : ""
            } ${diff?.changedItemIds?.has(item.id) ? styles.configValueGridItemChanged : ""}`}
          >
            <span className={styles.configValueGridLabel}>{item.label}</span>
            <div className={styles.configValueGridValue}>{item.value}</div>
          </div>
        ) : (
          <div
            key={item.id}
            className={`${styles.configValueGridGroup} ${
              item.wide ? styles.configValueGridItemWide : ""
            } ${diff?.changedItemIds?.has(item.id) ? styles.configValueGridItemChanged : ""}`}
          >
            <span className={styles.configValueGridGroupLabel}>{item.label}</span>
            <ConfigValueGrid items={item.children} nested />
          </div>
        ),
      )}
    </div>
  )
}

function changedValueItemClass(diff: ConfigValueDiffState | undefined, itemId: string): string {
  return diff?.changedItemIds?.has(itemId) ? styles.configValueCollectionItemChanged : ""
}

function FundamentalSmartContractsTable({
  contracts,
}: {
  readonly contracts: readonly FundamentalSmartContract[]
}) {
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const diff = useContext(ConfigValueDiffContext)

  return (
    <DataTable minWidth={0} variant="embedded">
      <DataTableTable aria-label="Fundamental smart contract addresses">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="2.5rem">#</DataTableHeaderCell>
            <DataTableHeaderCell>Address</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {contracts.length === 0 ? (
            <DataTableEmpty colSpan={2}>No fundamental smart contracts configured</DataTableEmpty>
          ) : (
            contracts.map((contract, index) => (
              <DataTableRow
                key={contract.address}
                className={changedValueItemClass(diff, `fundamental-contract-${contract.address}`)}
              >
                <DataTableCell columnWidth="2.5rem" className={styles.validatorIndex} tone="muted">
                  {index + 1}
                </DataTableCell>
                <DataTableCell truncate>
                  <ExplorerAddressChip
                    address={contract.address}
                    onAddressClick={(address, event) =>
                      openPath(routes.addressPath(address), event)
                    }
                    resolveName={false}
                    shorten={false}
                  />
                </DataTableCell>
              </DataTableRow>
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function PrecompiledContractsTable({
  contracts,
}: {
  readonly contracts: readonly PrecompiledContractConfiguration[]
}) {
  const diff = useContext(ConfigValueDiffContext)

  return (
    <DataTable minWidth={0} variant="embedded">
      <DataTableTable aria-label="Precompiled contracts">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="3.5rem">#</DataTableHeaderCell>
            <DataTableHeaderCell>Code hash</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="13rem">Gas usage</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {contracts.length === 0 ? (
            <DataTableEmpty colSpan={3}>No precompiled contracts configured</DataTableEmpty>
          ) : (
            contracts.map(contract => {
              const metadata = getPrecompiledContractMetadata(contract.codeHash)

              return (
                <DataTableRow
                  key={contract.codeHash}
                  className={changedValueItemClass(
                    diff,
                    `precompiled-contract-${contract.codeHash}`,
                  )}
                >
                  <DataTableCell className={styles.validatorIndex} tone="muted">
                    {contract.index + 1}
                  </DataTableCell>
                  <DataTableCell className={styles.validatorHash} truncate>
                    <span className={styles.precompiledContractHash}>
                      <TechnicalValue
                        copyLabel="precompiled contract code hash"
                        endLength={8}
                        startLength={8}
                        value={contract.codeHash}
                      />
                      <InfoPopover
                        ariaLabel={`About ${metadata.title}`}
                        contentClassName={styles.infoContent}
                      >
                        <p>
                          <strong>{metadata.title}</strong>
                        </p>
                        <p>{metadata.description}</p>
                        {metadata.verifiedContractUrl === undefined ? null : (
                          <a href={metadata.verifiedContractUrl} target="_blank" rel="noreferrer">
                            View verified contract
                            <ExternalLink size={13} aria-hidden="true" />
                          </a>
                        )}
                        {metadata.sourceUrl === undefined ? null : (
                          <a href={metadata.sourceUrl} target="_blank" rel="noreferrer">
                            View contract source
                            <ExternalLink size={13} aria-hidden="true" />
                          </a>
                        )}
                      </InfoPopover>
                    </span>
                  </DataTableCell>
                  <DataTableCell className={styles.validatorWeight}>
                    <ConfigTechnicalNumberValue
                      copyLabel="precompiled contract gas usage"
                      value={contract.gasUsage}
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

function ConfigPageSkeleton({navigationPosition}: {readonly navigationPosition: "left" | "right"}) {
  return (
    <div className={styles.loading} aria-busy="true">
      <div
        className={`${styles.loadingLayout} ${navigationPosition === "right" ? styles.navigationRight : ""}`}
      >
        <div className={styles.loadingIndex}>
          <Skeleton height="12px" width="36%" />
          {Array.from({length: 10}).map((_, index) => (
            <div key={index} className={styles.loadingIndexRow}>
              <Skeleton height="12px" width="24px" />
              <Skeleton height="12px" width={`${58 + (index % 3) * 12}%`} />
            </div>
          ))}
        </div>

        <div className={styles.loadingParameterList}>
          {Array.from({length: 4}).map((_, index) => (
            <article key={index} className={styles.loadingParameterCard}>
              <div className={styles.loadingParameterHeader}>
                <Skeleton height="26px" width="26px" radius="sm" />
                <Skeleton height="18px" width={`${42 + (index % 3) * 12}%`} />
                <Skeleton
                  className={styles.loadingParameterDescription}
                  height="13px"
                  width="72%"
                />
              </div>
              <div className={styles.loadingParameterBody}>
                <Skeleton height="34px" width="239px" radius="sm" />
                <SkeletonText lineCount={index % 2 === 0 ? 3 : 5} />
              </div>
            </article>
          ))}
        </div>
      </div>
    </div>
  )
}
