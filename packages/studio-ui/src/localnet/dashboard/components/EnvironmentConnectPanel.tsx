import {Button, CopyInlineButton, HighlightedCode, Tooltip} from "@acton/ui"
import {Braces, Cable, CircleAlert, FileCode2, Settings} from "lucide-react"
import {useEffect, useMemo, useState} from "react"
import {Link} from "react-router"

import styles from "./EnvironmentConnectPanel.module.css"

type IntegrationTarget = "acton" | "ton-client" | "rpc"

interface EnvironmentConnectPanelProps {
  readonly actonNetworkName: string
  readonly apiV2Url?: string
  readonly apiV3Url?: string
  readonly configureActonNetwork: boolean
  readonly controlUrl?: string
  readonly environmentName: string
  readonly explorerUrl?: string
  readonly integratePath: string
  readonly onDismiss?: () => void
  readonly settingsPath?: string
}

const integrationOptions = [
  {
    id: "acton",
    label: "Acton project",
    description: "Add this network to Acton.toml",
    icon: FileCode2,
  },
  {
    id: "ton-client",
    label: "JavaScript app",
    description: "Create an @ton/ton client",
    icon: Braces,
  },
  {
    id: "rpc",
    label: "RPC endpoints",
    description: "Connect any TON-compatible tool",
    icon: Cable,
  },
] as const satisfies readonly {
  readonly id: IntegrationTarget
  readonly label: string
  readonly description: string
  readonly icon: typeof Cable
}[]

export function EnvironmentConnectPanel({
  actonNetworkName,
  apiV2Url,
  apiV3Url,
  configureActonNetwork,
  controlUrl,
  environmentName,
  explorerUrl,
  integratePath,
  onDismiss,
  settingsPath,
}: EnvironmentConnectPanelProps) {
  const urls = useMemo(
    () => ({
      apiV2: toAbsoluteUrl(apiV2Url),
      apiV3: toAbsoluteUrl(apiV3Url),
      control: toAbsoluteUrl(controlUrl),
      explorer: toAbsoluteUrl(explorerUrl),
    }),
    [apiV2Url, apiV3Url, controlUrl, explorerUrl],
  )
  const endpointEntries = useMemo(
    () =>
      [
        urls.apiV2 ? {label: "V2 API", value: urls.apiV2} : undefined,
        urls.apiV3 ? {label: "V3 API", value: urls.apiV3} : undefined,
        urls.control ? {label: "Control API", value: urls.control} : undefined,
      ].filter((entry): entry is {readonly label: string; readonly value: string} =>
        Boolean(entry),
      ),
    [urls.apiV2, urls.apiV3, urls.control],
  )
  const availableOptions = useMemo(
    () =>
      integrationOptions.filter(option =>
        option.id === "rpc" ? endpointEntries.length > 0 : urls.apiV2 !== undefined,
      ),
    [endpointEntries.length, urls.apiV2],
  )
  const [target, setTarget] = useState<IntegrationTarget>(() => (urls.apiV2 ? "acton" : "rpc"))
  useEffect(() => {
    if (!availableOptions.some(option => option.id === target)) {
      setTarget(availableOptions[0]?.id ?? "rpc")
    }
  }, [availableOptions, target])

  const actonConfig = configureActonNetwork
    ? [
        `[networks.${actonNetworkName}]`,
        urls.apiV2 ? `api.v2 = "${urls.apiV2}"` : undefined,
        urls.apiV3 ? `api.v3 = "${urls.apiV3}"` : undefined,
        urls.explorer ? `explorer = "${urls.explorer}"` : undefined,
      ]
        .filter((line): line is string => Boolean(line))
        .join("\n")
    : undefined
  const tonClientSetup = `import { TonClient } from "@ton/ton"

const client = new TonClient({
  endpoint: "${withoutTrailingSlash(urls.apiV2 ?? "")}/jsonRPC",
})`
  const actonRunCommand = `acton script --net ${actonNetworkName} scripts/deploy.tolk`
  const tonClientRequest = `const masterchain = await client.getMasterchainInfo()
console.log(masterchain)`
  const integrationPrompt = integrationPromptFor({
    actonConfig,
    actonRunCommand,
    actonNetworkName,
    environmentName,
    target,
    tonClientSetup,
    tonClientRequest,
    urls,
  })
  return (
    <section
      className={styles.panel}
      aria-labelledby="connect-environment-title"
      data-dismissible={onDismiss ? true : undefined}
    >
      <header className={styles.header}>
        <h2 id="connect-environment-title">Connect environment</h2>
        <div className={styles.headerActions}>
          {settingsPath ? (
            <Link className={styles.configureLink} to={settingsPath}>
              <Settings size={15} aria-hidden="true" />
              Configure
            </Link>
          ) : undefined}
          <CopyInlineButton
            className={styles.promptButton}
            value={integrationPrompt}
            variant="default"
            label="Copy integration prompt"
            copiedLabel="Integration prompt copied"
          >
            Copy integration prompt
          </CopyInlineButton>
        </div>
      </header>

      <div className={styles.setup}>
        <div className={styles.step}>
          <span className={styles.stepNumber}>1</span>
          <div className={styles.stepContent}>
            <h3>Choose what to connect</h3>
            <div className={styles.integrationOptions}>
              {availableOptions.map(option => {
                const Icon = option.icon
                const selected = target === option.id
                const description =
                  option.id === "acton" && !configureActonNetwork
                    ? `Use Acton's built-in ${actonNetworkName} network`
                    : option.description

                return (
                  <button
                    key={option.id}
                    type="button"
                    className={styles.integrationOption}
                    data-selected={selected || undefined}
                    aria-pressed={selected}
                    onClick={() => setTarget(option.id)}
                  >
                    <Icon size={17} aria-hidden="true" />
                    <span>
                      <strong>{option.label}</strong>
                      <small>{description}</small>
                    </span>
                  </button>
                )
              })}
            </div>
          </div>
        </div>

        <div className={styles.step}>
          <span className={styles.stepNumber}>2</span>
          <div className={styles.stepContent}>
            <div className={styles.outputHeader}>
              <div>
                <h3>
                  {target === "rpc"
                    ? "Use an endpoint"
                    : target === "acton" && !actonConfig
                      ? "Run a script on this network"
                      : "Add this setup"}
                </h3>
                {target === "acton" && !actonConfig ? (
                  <p className={styles.stepDescription}>
                    Studio routes Acton&apos;s built-in {actonNetworkName} network while it is
                    running
                  </p>
                ) : undefined}
              </div>
              {target === "acton" ? (
                <CopyInlineButton
                  value={actonConfig ?? actonRunCommand}
                  label={actonConfig ? "Copy Acton configuration" : "Copy Acton command"}
                  copiedLabel={actonConfig ? "Acton configuration copied" : "Acton command copied"}
                >
                  Copy
                </CopyInlineButton>
              ) : target === "ton-client" ? (
                <CopyInlineButton
                  value={tonClientSetup}
                  label="Copy TonClient setup"
                  copiedLabel="TonClient setup copied"
                >
                  Copy
                </CopyInlineButton>
              ) : undefined}
            </div>

            {target === "rpc" ? (
              <div className={styles.endpointList}>
                {endpointEntries.map(endpoint => (
                  <EndpointRow key={endpoint.label} {...endpoint} />
                ))}
              </div>
            ) : target === "acton" && !actonConfig ? (
              <HighlightedCode
                className={styles.codeBlock}
                language="shellscript"
                value={actonRunCommand}
                ariaLabel="Acton script command"
              />
            ) : (
              <HighlightedCode
                className={styles.codeBlock}
                language={target === "acton" ? "toml" : "javascript"}
                value={target === "acton" ? (actonConfig ?? "") : tonClientSetup}
                ariaLabel={target === "acton" ? "Acton TOML configuration" : "JavaScript setup"}
              />
            )}
          </div>
        </div>

        {target === "acton" && actonConfig ? (
          <div className={styles.step}>
            <span className={styles.stepNumber}>3</span>
            <div className={styles.stepContent}>
              <div className={styles.outputHeader}>
                <h3>Run a script on this network</h3>
                <CopyInlineButton
                  value={actonRunCommand}
                  label="Copy Acton command"
                  copiedLabel="Acton command copied"
                >
                  Copy
                </CopyInlineButton>
              </div>
              <HighlightedCode
                className={styles.codeBlock}
                language="shellscript"
                value={actonRunCommand}
                ariaLabel="Acton script command"
              />
            </div>
          </div>
        ) : undefined}

        {target === "ton-client" ? (
          <div className={styles.step}>
            <span className={styles.stepNumber}>3</span>
            <div className={styles.stepContent}>
              <div className={styles.outputHeader}>
                <div>
                  <h3>Make a request</h3>
                  <p className={styles.stepDescription}>
                    Requests use the same TonClient API as testnet and mainnet
                  </p>
                </div>
                <CopyInlineButton
                  value={tonClientRequest}
                  label="Copy JavaScript request"
                  copiedLabel="JavaScript request copied"
                >
                  Copy
                </CopyInlineButton>
              </div>
              <HighlightedCode
                className={styles.codeBlock}
                language="javascript"
                value={tonClientRequest}
                ariaLabel="JavaScript request example"
              />
            </div>
          </div>
        ) : undefined}
      </div>

      {onDismiss ? (
        <footer className={styles.dismissFooter}>
          <span className={styles.dismissHint}>
            <CircleAlert size={14} aria-hidden="true" />
            <span>
              This setup remains available on the <Link to={integratePath}>Integrate</Link> page
            </span>
          </span>
          <Button type="button" variant="primary" size="sm" onClick={onDismiss}>
            Finish setup
          </Button>
        </footer>
      ) : undefined}
    </section>
  )
}

function EndpointRow({label, value}: {readonly label: string; readonly value: string}) {
  return (
    <div className={styles.endpointRow}>
      <span>{label}</span>
      <Tooltip content={value}>
        <code>{value}</code>
      </Tooltip>
      <CopyInlineButton
        value={value}
        label={`Copy ${label} endpoint`}
        copiedLabel={`${label} endpoint copied`}
        copiedChildren={null}
      >
        {null}
      </CopyInlineButton>
    </div>
  )
}

function integrationPromptFor({
  actonConfig,
  actonRunCommand,
  actonNetworkName,
  environmentName,
  target,
  tonClientSetup,
  tonClientRequest,
  urls,
}: {
  readonly actonConfig?: string
  readonly actonRunCommand: string
  readonly actonNetworkName: string
  readonly environmentName: string
  readonly target: IntegrationTarget
  readonly tonClientSetup: string
  readonly tonClientRequest: string
  readonly urls: {
    readonly apiV2?: string
    readonly apiV3?: string
    readonly control?: string
    readonly explorer?: string
  }
}): string {
  if (target === "acton") {
    if (!actonConfig) {
      return `Run an Acton script on "${environmentName}". Studio routes the built-in ${actonNetworkName} network while it is running:

${actonRunCommand}`
    }

    return `Connect the Acton project to "${environmentName}" by adding this configuration to Acton.toml:

${actonConfig}

Then run a script on the localnet:

${actonRunCommand}`
  }

  if (target === "ton-client") {
    return `Connect the JavaScript application to "${environmentName}" with @ton/ton:

${tonClientSetup}

Make requests with the same TonClient API as testnet and mainnet:

${tonClientRequest}`
  }

  const endpointLines = [
    urls.apiV2 ? `V2 API: ${urls.apiV2}` : undefined,
    urls.apiV3 ? `V3 API: ${urls.apiV3}` : undefined,
    urls.control ? `Control API: ${urls.control}` : undefined,
    urls.explorer ? `Explorer: ${urls.explorer}` : undefined,
  ].filter((line): line is string => Boolean(line))

  return `Connect the TON application to "${environmentName}" through Acton Studio.

${endpointLines.join("\n")}`
}

function toAbsoluteUrl(value: string | undefined): string | undefined {
  if (!value) return undefined
  try {
    return new URL(value, globalThis.location.origin).href
  } catch {
    return value
  }
}

function withoutTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value
}
