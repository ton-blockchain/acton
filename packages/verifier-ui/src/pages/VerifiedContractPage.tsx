import {
  Button,
  CodeViewer,
  CountValue,
  CopyInlineAction,
  DateTime,
  HighlightedCode,
  NumberValue,
  SourceLocationValue,
  TechnicalValue,
  shortenMiddle,
} from "@acton/ui"
import {useEffect, useMemo, useState, type ReactNode} from "react"
import {Download, ExternalLink} from "lucide-react"

import {StatusPill} from "../components/StatusPill"
import compilerIcon from "../assets/ton-verifier-icons/compiler.svg"
import contractIcon from "../assets/ton-verifier-icons/contract.svg"
import verificationIcon from "../assets/ton-verifier-icons/verification.svg"
import verificationAlertIcon from "../assets/ton-verifier-icons/verification-alert.svg"
import verificationBinaryIcon from "../assets/ton-verifier-icons/verification-binary.svg"
import verificationBombIcon from "../assets/ton-verifier-icons/verification-bomb.svg"
import verificationPaperIcon from "../assets/ton-verifier-icons/verification-paper.svg"
import verifiedSourceIcon from "../assets/ton-verifier-icons/verified-light.svg"
import type {VerificationSourceResponse, VerifierApi} from "../lib/api"
import {downloadSourceArchive} from "../lib/source-archive"
import {parseLookupTarget, type LookupTarget} from "../lib/target"
import detailsStyles from "./ContractDetails.module.css"
import summaryStyles from "./ContractSummary.module.css"
import styles from "./VerifiedContractPage.module.css"

function DetailRow({
  label,
  value,
  href,
  copyable = true,
}: {
  readonly label: string
  readonly value: ReactNode
  readonly href?: string
  readonly copyable?: boolean
}) {
  return (
    <div className={detailsStyles.detailRow}>
      <dt>{label}</dt>
      <dd>
        {href ? (
          <a
            className={styles.externalLink}
            href={href}
            target="_blank"
            rel="noreferrer"
            title={`View ${label.toLowerCase()} tag on GitHub`}
          >
            <span>{value}</span>
            <ExternalLink size={13} aria-hidden="true" />
          </a>
        ) : typeof value === "string" ? (
          <span className={detailsStyles.detailText} title={value}>
            {value}
          </span>
        ) : (
          value
        )}
        {copyable && typeof value === "string" && value && (
          <CopyInlineAction value={value} label={`Copy ${label}`} copiedLabel={`${label} copied`} />
        )}
      </dd>
    </div>
  )
}

const compilerTagSources: Readonly<
  Record<string, {readonly repositoryUrl: string; readonly tagPrefix: string}>
> = {
  tact: {
    repositoryUrl: "https://github.com/tact-lang/tact",
    tagPrefix: "v",
  },
  tolk: {
    repositoryUrl: "https://github.com/ton-blockchain/ton",
    tagPrefix: "tolk-",
  },
}

function compilerVersionUrl(language: string, version: string): string | undefined {
  const source = compilerTagSources[language.trim().toLowerCase()]
  const normalizedVersion = version.trim()
  if (!source) {
    return undefined
  }
  if (!normalizedVersion) {
    return undefined
  }

  const tag = normalizedVersion.startsWith(source.tagPrefix)
    ? normalizedVersion
    : `${source.tagPrefix}${normalizedVersion}`
  return `${source.repositoryUrl}/releases/tag/${encodeURIComponent(tag)}`
}

function paymentTransactionUrl(transactionHash: string): string {
  return `https://actonscan.com/tx/${encodeURIComponent(transactionHash)}?network=testnet`
}

function PaymentTransactionLink({transactionHash}: {readonly transactionHash: string}) {
  return (
    <a
      className={styles.externalLink}
      href={paymentTransactionUrl(transactionHash)}
      target="_blank"
      rel="noreferrer"
      title="View payment transaction on Actonscan"
      aria-label={`View payment transaction ${transactionHash} on Actonscan`}
    >
      <TechnicalValue copyable={false} tooltip={false} value={transactionHash} />
      <ExternalLink size={13} aria-hidden="true" />
    </a>
  )
}

function CompilerVersionLink({
  language,
  version,
}: {
  readonly language: string
  readonly version: string
}) {
  const href = compilerVersionUrl(language, version)

  if (!href) {
    return version
  }

  return (
    <a
      className={styles.externalLink}
      href={href}
      target="_blank"
      rel="noreferrer"
      title={`View ${language} ${version} tag on GitHub`}
    >
      <span>{version}</span>
      <ExternalLink size={13} aria-hidden="true" />
    </a>
  )
}

function PanelHeading({
  icon,
  label,
  title,
  titleLevel = "h2",
}: {
  readonly icon: string
  readonly label: string
  readonly title: string
  readonly titleLevel?: "h1" | "h2"
}) {
  const Title = titleLevel

  return (
    <div className={summaryStyles.panelHeading}>
      <img className={summaryStyles.panelHeadingIcon} src={icon} alt="" aria-hidden="true" />
      <div>
        <span>{label}</span>
        <Title>{title}</Title>
      </div>
    </div>
  )
}

const verificationPoints = [
  {
    icon: verificationBinaryIcon,
    text: "This source code compiles to the same exact bytecode that is found on-chain.",
  },
  {
    icon: verificationPaperIcon,
    text: "You can review the stored source bundle and perform your own client-side verification.",
  },
  {
    icon: verificationAlertIcon,
    text: "Variable/function names may not reflect actual usage. compiler may remove unused code.",
  },
  {
    icon: verificationBombIcon,
    text: "Comments may not be honest and should generally be ignored.",
  },
] as const

function VerificationExplainer() {
  return (
    <section className={summaryStyles.summaryProof} aria-label="How this contract is verified">
      <PanelHeading
        icon={verificationIcon}
        label="Verification"
        title="How is this contract verified?"
      />
      <div className={summaryStyles.verificationPointGrid}>
        {verificationPoints.map(point => (
          <div className={summaryStyles.verificationPoint} key={point.text}>
            <img
              className={summaryStyles.verificationPointIcon}
              src={point.icon}
              alt=""
              aria-hidden="true"
            />
            <p>{point.text}</p>
          </div>
        ))}
      </div>
    </section>
  )
}

function lookupAddress(lookupTarget: LookupTarget | undefined): string | undefined {
  return lookupTarget?.kind === "address" ? lookupTarget.value : undefined
}

function VerifiedContract({
  data,
  lookupTarget,
  selectedSourcePath,
  onSelectedSourcePathChange,
}: {
  readonly data: VerificationSourceResponse
  readonly lookupTarget: LookupTarget | undefined
  readonly selectedSourcePath?: string
  readonly onSelectedSourcePathChange?: (path: string) => void
}) {
  const address = lookupAddress(lookupTarget)
  const {bundle} = data

  if (!bundle) {
    return (
      <section className={styles["empty-state"]}>
        <StatusPill verified={false} />
        <h2>Contract is indexed, but no verified source bundle is available.</h2>
      </section>
    )
  }

  const verifiedAt =
    Number.isFinite(bundle.verified_at) && bundle.verified_at > 0 ? bundle.verified_at : undefined
  const compactCompilerParams = JSON.stringify(bundle.compiler.params)
  const readableCompilerParams =
    compactCompilerParams.length <= 96
      ? compactCompilerParams
          .replaceAll(":", ": ")
          .replaceAll(",", ", ")
          .replace(/^\{/, "{ ")
          .replace(/\}$/, " }")
      : compactCompilerParams
  const compilerParams =
    readableCompilerParams.length <= 112
      ? readableCompilerParams
      : JSON.stringify(bundle.compiler.params, null, 2)
  return (
    <>
      <section className={summaryStyles.contractSummary}>
        <div className={summaryStyles.summaryMain}>
          <PanelHeading
            icon={contractIcon}
            label="Contract"
            title={address ? shortenMiddle(address, {start: 18, end: 12}) : "Verified code hash"}
            titleLevel="h1"
          />
          <div className={summaryStyles.summaryStatusRow}>
            <StatusPill verified={data.verified} />
          </div>
          <div className={summaryStyles.summaryFacts}>
            <div className={summaryStyles.summaryFact}>
              <span>Language</span>
              <strong>{bundle.compiler.language}</strong>
            </div>
            <div className={summaryStyles.summaryFact}>
              <span>Compiler</span>
              <strong>
                <CompilerVersionLink
                  language={bundle.compiler.language}
                  version={bundle.compiler.version}
                />
              </strong>
            </div>
            <div className={summaryStyles.summaryFact}>
              <span>Files</span>
              <strong>
                <NumberValue value={bundle.files.length} />
              </strong>
            </div>
          </div>
          <div className={summaryStyles.hashCard}>
            <span>Verified code hash</span>
            <p title={data.code_hash}>{data.code_hash}</p>
          </div>
        </div>
        <VerificationExplainer />
      </section>

      <div className={detailsStyles.layout}>
        <section className={detailsStyles.panel} aria-labelledby="verification-metadata-title">
          <div className={detailsStyles.panelHeading}>
            <img
              className={`${summaryStyles.panelHeadingIcon} ${summaryStyles.compact}`}
              src={compilerIcon}
              alt=""
              aria-hidden="true"
            />
            <h2 id="verification-metadata-title">Verification metadata</h2>
          </div>
          <dl>
            {address && (
              <DetailRow
                label="Address"
                value={
                  <TechnicalValue
                    copyLabel="address"
                    copyVisibility="always"
                    shorten={false}
                    value={address}
                  />
                }
              />
            )}
            {verifiedAt && (
              <DetailRow
                label="Verified at"
                value={<DateTime display="date-time-seconds" unit="seconds" value={verifiedAt} />}
                copyable={false}
              />
            )}
            <DetailRow
              label="Code hash"
              value={
                <TechnicalValue
                  copyLabel="code hash"
                  copyVisibility="always"
                  shorten={false}
                  value={data.code_hash}
                />
              }
            />
            <DetailRow
              label="Bundle hash"
              value={
                <TechnicalValue
                  copyLabel="bundle hash"
                  copyVisibility="always"
                  shorten={false}
                  value={bundle.source_bundle_hash}
                />
              }
            />
            {bundle.storage_revision && (
              <DetailRow
                label="Storage revision"
                value={
                  <TechnicalValue
                    copyLabel="storage revision"
                    copyVisibility="always"
                    shorten={false}
                    value={bundle.storage_revision}
                  />
                }
              />
            )}
            <DetailRow label="Language" value={bundle.compiler.language} copyable={false} />
            <DetailRow
              label="Compiler"
              value={bundle.compiler.version}
              href={compilerVersionUrl(bundle.compiler.language, bundle.compiler.version)}
              copyable={false}
            />
            <DetailRow
              label="Entrypoint"
              value={
                <SourceLocationValue
                  copyable
                  copyVisibility="always"
                  maxSegments={Number.MAX_SAFE_INTEGER}
                  value={{file: bundle.entrypoint}}
                />
              }
            />
            {bundle.payment_tx_hash && (
              <DetailRow
                label="Payment tx"
                value={<PaymentTransactionLink transactionHash={bundle.payment_tx_hash} />}
                copyable={false}
              />
            )}
          </dl>
          <div className={detailsStyles.metadataJson}>
            <div className={detailsStyles.metadataJsonTitle}>Compile params</div>
            <HighlightedCode
              className={detailsStyles.highlightedJson}
              value={compilerParams}
              language="json"
            />
          </div>
        </section>

        <section className={detailsStyles.sourceSection}>
          <div className={detailsStyles.sectionHeader}>
            <div className={detailsStyles.sectionTitle}>
              <img
                className={`${summaryStyles.panelHeadingIcon} ${summaryStyles.compact}`}
                src={verifiedSourceIcon}
                alt=""
                aria-hidden="true"
              />
              <div>
                <h2>Source bundle</h2>
                <span className={detailsStyles.fileCount}>
                  <CountValue singular="file" value={bundle.files.length} />
                </span>
              </div>
            </div>
            <div className={detailsStyles.sectionActions}>
              <Button
                size="sm"
                variant="outline"
                leadingIcon={<Download size={15} />}
                onClick={() => downloadSourceArchive(bundle)}
              >
                Download sources
              </Button>
            </div>
          </div>
          <CodeViewer
            key={bundle.source_bundle_hash}
            className={detailsStyles.sourceCodeViewer}
            defaultSelectedPath={selectedSourcePath}
            emptyMessage="No source files stored for this bundle"
            files={bundle.files}
            entrypoint={bundle.entrypoint}
            onSelectedPathChange={onSelectedSourcePathChange}
          />
        </section>
      </div>
    </>
  )
}

function UnverifiedContract({
  data,
  lookupTarget,
}: {
  readonly data: VerificationSourceResponse
  readonly lookupTarget: LookupTarget | undefined
}) {
  const address = lookupAddress(lookupTarget)

  return (
    <section className={styles["empty-state"]}>
      <StatusPill verified={false} />
      <h1>Contract is not verified</h1>
      <p>
        This target resolves to code hash <code className={styles.codeHash}>{data.code_hash}</code>,
        but there is no stored source bundle for it
      </p>
      {address && (
        <dl className={detailsStyles.summaryGrid}>
          <DetailRow
            label="Address"
            value={<TechnicalValue copyLabel="address" value={address} />}
          />
        </dl>
      )}
    </section>
  )
}

export interface VerifiedContractPageProps {
  readonly api: VerifierApi
  readonly target: string
  readonly selectedSourcePath?: string
  readonly onSelectedSourcePathChange?: (path: string) => void
  readonly className?: string
}

export function VerifiedContractPage({
  api,
  target,
  selectedSourcePath,
  onSelectedSourcePathChange,
  className,
}: VerifiedContractPageProps) {
  const rawLookup = target.trim()
  const lookupTarget = useMemo(() => {
    try {
      return parseLookupTarget(rawLookup)
    } catch {
      return undefined
    }
  }, [rawLookup])
  const [data, setData] = useState<VerificationSourceResponse | undefined>()
  const [error, setError] = useState<string | undefined>()
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false

    const load = async () => {
      setLoading(true)
      setError(undefined)
      setData(undefined)
      try {
        const target = parseLookupTarget(rawLookup)
        const result = await api.fetchVerificationSource(target)
        if (!cancelled) {
          setData(result)
        }
      } catch (error) {
        if (!cancelled) {
          setError(error instanceof Error ? error.message : String(error))
        }
      } finally {
        if (!cancelled) {
          setLoading(false)
        }
      }
    }

    void load()
    return () => {
      cancelled = true
    }
  }, [api, rawLookup])

  return (
    <div className={`${styles.page} ${className ?? ""}`}>
      {loading ? (
        <section className={styles["loading-state"]}>Loading verification state...</section>
      ) : error ? (
        <section className={`${styles["empty-state"]} ${styles["error-state"]}`}>
          <h1>Could not load contract</h1>
          <p>{error}</p>
        </section>
      ) : data?.verified ? (
        <VerifiedContract
          data={data}
          lookupTarget={lookupTarget}
          selectedSourcePath={selectedSourcePath}
          onSelectedSourcePathChange={onSelectedSourcePathChange}
        />
      ) : data ? (
        <UnverifiedContract data={data} lookupTarget={lookupTarget} />
      ) : null}
    </div>
  )
}
