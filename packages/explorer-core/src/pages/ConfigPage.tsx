import {
  ByteSize,
  BooleanValue,
  ContentTabs,
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
import {ChevronDown, ExternalLink, Link2, Search} from "lucide-react"
import {useEffect, useMemo, useState, type FC, type ReactNode} from "react"
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
  type ValidatorSetConfiguration,
} from "../api/config"
import {getExtraCurrencyMetadata} from "../api/extraCurrency"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {GlobalCapabilities} from "../components/GlobalCapabilities"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useOpenExplorerPath} from "../hooks/useOpenExplorerPath"
import styles from "./ConfigPage.module.css"

interface ConfigPageProps {
  readonly client: TonClient
}

type ConfigLoadState =
  | {readonly status: "loading"}
  | {readonly status: "success"; readonly config: NetworkConfig}
  | {readonly status: "error"; readonly message: string}

const TON_CONFIG_DOC_ANCHORS: Readonly<Record<number, string>> = {
  0: "param-0-config-address",
  1: "param-1-elector-address",
  2: "param-2-ton-minting-address",
  3: "param-3-fee-collector-address",
  4: "param-4-root-dns-address",
  6: "param-6-extra-currency-minting-prices",
  7: "param-7-extra-currency-volume",
  8: "param-8-network-version",
  9: "param-9-mandatory-params",
  10: "param-10-critical-params",
  11: "param-11-config-params",
  12: "param-12-workchain-config",
  13: "param-13-complaint-cost",
  14: "param-14-block-reward",
  15: "param-15-elections-timing",
  16: "param-16-validators-limits",
  17: "param-17-stake-limits",
  18: "param-18-storage-prices",
  20: "param-20-and-21-gas-prices",
  21: "param-20-and-21-gas-prices",
  22: "param-22-and-23-block-limits",
  23: "param-22-and-23-block-limits",
  24: "param-24-and-25-message-price",
  25: "param-24-and-25-message-price",
  28: "param-28-catchain-config",
  29: "param-29-consensus-config",
  30: "param-30-consensus-extension",
  31: "param-31-fee-exempt-contracts",
  32: "param-32-34-and-36-validator-lists",
  33: "param-32-34-and-36-validator-lists",
  34: "param-32-34-and-36-validator-lists",
  35: "param-32-34-and-36-validator-lists",
  36: "param-32-34-and-36-validator-lists",
  37: "param-32-34-and-36-validator-lists",
  40: "param-40-misbehavior-punishment",
  43: "param-43-account-and-message-limits",
  44: "param-44-suspended-addresses",
  45: "param-45-precompiled-contracts",
  71: "param-71---73-outbound-bridges",
  72: "param-71---73-outbound-bridges",
  73: "param-71---73-outbound-bridges",
  79: "param-79-81-and-82-inbound-bridges",
  81: "param-79-81-and-82-inbound-bridges",
  82: "param-79-81-and-82-inbound-bridges",
}

function tonConfigDocsHref(id: number): string {
  const anchor = id < 0 ? "negative-parameters" : TON_CONFIG_DOC_ANCHORS[id]
  return anchor === undefined ? TON_CONFIG_DOCS_URL : `${TON_CONFIG_DOCS_URL}#${anchor}`
}

export const ConfigPage: FC<ConfigPageProps> = ({client}) => {
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
        return
      }
      try {
        const config = await client.getNetworkConfig(seqno)
        if (active) setLoadState({status: "success", config})
      } catch (error) {
        if (active) {
          setLoadState({
            status: "error",
            message: error instanceof Error ? error.message : String(error),
          })
        }
      }
    }

    void load()
    return () => {
      active = false
    }
  }, [client, seqno, seqnoParam])

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

  useEffect(() => {
    if (!config || !globalThis.location.hash) return

    const anchorId = decodeConfigAnchor(globalThis.location.hash)
    if (!anchorId) return

    const frame = globalThis.requestAnimationFrame(() => {
      globalThis.document
        .getElementById(anchorId)
        ?.scrollIntoView({behavior: "smooth", block: "start"})
    })

    return () => globalThis.cancelAnimationFrame(frame)
  }, [config, visibleParameters])

  return (
    <section className={styles.container}>
      <header className={styles.header}>
        <ExplorerBreadcrumbs
          items={
            seqno === undefined
              ? [{label: "Config"}]
              : [{label: "Config", path: routes.configPath()}, {label: `Block #${seqno}`}]
          }
        />
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

      {loadState.status === "loading" ? (
        <ConfigPageSkeleton />
      ) : loadState.status === "error" ? (
        <section className={styles.error} role="alert">
          <h2>Network configuration is unavailable</h2>
          <p>{loadState.message}</p>
        </section>
      ) : (
        <ConfigContent visibleParameters={visibleParameters} />
      )}
    </section>
  )
}

function decodeConfigAnchor(hash: string): string | undefined {
  try {
    const anchorId = decodeURIComponent(hash.slice(1))
    return anchorId || undefined
  } catch {
    return undefined
  }
}

function parseConfigSeqno(value: string | undefined): number | undefined {
  if (value === undefined || !/^\d+$/.test(value)) return undefined

  const seqno = Number(value)
  return Number.isSafeInteger(seqno) ? seqno : undefined
}

function ConfigContent({
  visibleParameters,
}: {
  readonly visibleParameters: readonly NetworkConfigParameter[]
}) {
  return (
    <>
      <div className={styles.configLayout}>
        <aside className={styles.indexPanel} aria-label="Configuration parameter index">
          <nav className={styles.indexList}>
            {visibleParameters.map(parameter => (
              <a
                key={parameter.id}
                className={styles.indexLink}
                href={`#config-parameter-${parameter.id}`}
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
              <ConfigParameterCard key={parameter.id} parameter={parameter} />
            ))
          )}
        </main>
      </div>
    </>
  )
}

function ConfigParameterCard({parameter}: {readonly parameter: NetworkConfigParameter}) {
  const hasValueTab = parameter.parsedValue !== undefined
  const hasCompactValue =
    parameter.address !== undefined ||
    parameter.burningConfiguration !== undefined ||
    parameter.extraCurrencies !== undefined ||
    parameter.globalVersion !== undefined ||
    parameter.configurationValues !== undefined ||
    parameter.globalId !== undefined ||
    parameter.parameterIds !== undefined ||
    parameter.fundamentalSmartContracts !== undefined ||
    parameter.precompiledContracts !== undefined ||
    parameter.validatorSet !== undefined ||
    parameter.suspendedAddresses !== undefined ||
    parameter.bridgeConfiguration !== undefined
  const [activeTab, setActiveTab] = useState<"raw" | "value">(hasValueTab ? "value" : "raw")
  const tabs = hasValueTab
    ? [
        {label: "Value", value: "value" as const},
        {label: "Raw cell", value: "raw" as const},
      ]
    : [{label: "Raw cell", value: "raw" as const}]

  return (
    <article id={`config-parameter-${parameter.id}`} className={styles.parameterCard}>
      <header className={styles.parameterHeader}>
        <ConfigParameterAnchor id={parameter.id} className={styles.parameterId} />
        <div className={styles.parameterHeading}>
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
          <p className={styles.parameterDescription}>{parameter.description}</p>
        </div>
      </header>

      <ContentTabs
        ariaLabel={`Views for configuration parameter ${parameter.id}`}
        className={styles.parameterTabs}
        onValueChange={setActiveTab}
        panelClassName={`${styles.parameterTabPanel} ${
          activeTab === "raw" || (activeTab === "value" && hasCompactValue)
            ? styles.compactParameterTabPanel
            : ""
        }`}
        tabs={tabs}
        value={activeTab}
      >
        {activeTab === "value" ? (
          <ConfigParameterValue parameter={parameter} />
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
    >
      <span className={styles.parameterAnchorNumber}>{id}</span>
      <Link2 className={styles.parameterAnchorIcon} size={16} aria-hidden="true" />
    </a>
  )

  return tooltip === undefined ? anchor : <Tooltip content={tooltip}>{anchor}</Tooltip>
}

function ConfigParameterValue({parameter}: {readonly parameter: NetworkConfigParameter}) {
  if (parameter.address !== undefined) {
    return (
      <div className={styles.parsedValue}>
        <ConfigAddressValue address={parameter.address} />
      </div>
    )
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
                <DataTableRow key={validator.index}>
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
                  <DataTableRow key={address}>
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
                  label: "External chain address",
                  value: (
                    <TechnicalValue
                      value={configuration.externalChainAddress}
                      copyLabel="external chain address"
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
      <BridgeOracleTable oracles={configuration.oracles} />
    </div>
  )
}

function BridgeOracleTable({oracles}: {readonly oracles: readonly BridgeOracle[]}) {
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
            <DataTableHeaderCell columnWidth="3.5rem">#</DataTableHeaderCell>
            <DataTableHeaderCell>Key</DataTableHeaderCell>
            <DataTableHeaderCell>Value</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {oracles.length === 0 ? (
            <DataTableEmpty colSpan={3}>No oracles configured</DataTableEmpty>
          ) : (
            oracles.map(oracle => (
              <DataTableRow key={oracle.index}>
                <DataTableCell className={styles.validatorIndex} tone="muted">
                  {oracle.index + 1}
                </DataTableCell>
                <DataTableCell className={styles.validatorHash} truncate>
                  <TechnicalValue
                    copyLabel="oracle key"
                    endLength={8}
                    startLength={8}
                    value={oracle.key}
                  />
                </DataTableCell>
                <DataTableCell className={styles.validatorHash} truncate>
                  <TechnicalValue
                    copyLabel="oracle value"
                    endLength={8}
                    startLength={8}
                    value={oracle.value}
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
                className={styles.configParameterIdAnchor}
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
  return (
    <div className={`${styles.configValueGrid} ${nested ? styles.configValueGridNested : ""}`}>
      {items.map(item =>
        item.children === undefined ? (
          <div
            key={item.id}
            className={`${styles.configValueGridItem} ${item.wide ? styles.configValueGridItemWide : ""}`}
          >
            <span className={styles.configValueGridLabel}>{item.label}</span>
            <div className={styles.configValueGridValue}>{item.value}</div>
          </div>
        ) : (
          <div
            key={item.id}
            className={`${styles.configValueGridGroup} ${item.wide ? styles.configValueGridItemWide : ""}`}
          >
            <span className={styles.configValueGridGroupLabel}>{item.label}</span>
            <ConfigValueGrid items={item.children} nested />
          </div>
        ),
      )}
    </div>
  )
}

function FundamentalSmartContractsTable({
  contracts,
}: {
  readonly contracts: readonly FundamentalSmartContract[]
}) {
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()

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
              <DataTableRow key={contract.address}>
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
            contracts.map(contract => (
              <DataTableRow key={contract.codeHash}>
                <DataTableCell className={styles.validatorIndex} tone="muted">
                  {contract.index + 1}
                </DataTableCell>
                <DataTableCell className={styles.validatorHash} truncate>
                  <TechnicalValue
                    copyLabel="precompiled contract code hash"
                    endLength={8}
                    startLength={8}
                    value={contract.codeHash}
                  />
                </DataTableCell>
                <DataTableCell className={styles.validatorWeight}>
                  <ConfigTechnicalNumberValue
                    copyLabel="precompiled contract gas usage"
                    value={contract.gasUsage}
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

function ConfigPageSkeleton() {
  return (
    <div className={styles.loading} aria-busy="true">
      <div className={styles.loadingLayout}>
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
                <Skeleton height="30px" width="30px" radius="sm" />
                <div className={styles.loadingParameterHeading}>
                  <Skeleton height="18px" width={`${42 + (index % 3) * 12}%`} />
                  <Skeleton height="13px" width="72%" />
                </div>
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
