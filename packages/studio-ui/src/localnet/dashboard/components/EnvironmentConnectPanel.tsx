import {CopyInlineButton, HighlightedCode, Tooltip} from "@acton/ui"
import {Braces, Cable, FileCode2, Settings} from "lucide-react"
import {useMemo, useState} from "react"
import {Link} from "react-router"

import styles from "./EnvironmentConnectPanel.module.css"

type IntegrationTarget = "acton" | "ton-client" | "rpc"

interface EnvironmentConnectPanelProps {
  readonly apiV2Url: string
  readonly apiV3Url: string
  readonly controlUrl: string
  readonly environmentName: string
  readonly explorerUrl: string
  readonly rpcUrl: string
  readonly settingsPath: string
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
  apiV2Url,
  apiV3Url,
  controlUrl,
  environmentName,
  explorerUrl,
  rpcUrl,
  settingsPath,
}: EnvironmentConnectPanelProps) {
  const [target, setTarget] = useState<IntegrationTarget>("acton")
  const urls = useMemo(
    () => ({
      apiV2: toAbsoluteUrl(apiV2Url),
      apiV3: toAbsoluteUrl(apiV3Url),
      control: toAbsoluteUrl(controlUrl),
      explorer: toAbsoluteUrl(explorerUrl),
      rpc: toAbsoluteUrl(rpcUrl),
    }),
    [apiV2Url, apiV3Url, controlUrl, explorerUrl, rpcUrl],
  )
  const actonConfig = `[networks.localnet]
api.v2 = "${urls.apiV2}"
api.v3 = "${urls.apiV3}"
explorer = "${urls.explorer}"`
  const tonClientSetup = `import { TonClient } from "@ton/ton"

const client = new TonClient({
  endpoint: "${withoutTrailingSlash(urls.apiV2)}/jsonRPC",
})`
  const actonRunCommand = "acton script --net localnet scripts/deploy.tolk"
  const tonClientRequest = `const masterchain = await client.getMasterchainInfo()
console.log(masterchain)`
  const integrationPrompt = integrationPromptFor({
    actonConfig,
    actonRunCommand,
    environmentName,
    target,
    tonClientSetup,
    tonClientRequest,
    urls,
  })

  return (
    <section className={styles.panel} aria-labelledby="connect-environment-title">
      <header className={styles.header}>
        <h2 id="connect-environment-title">Connect environment</h2>
        <div className={styles.headerActions}>
          <Link className={styles.configureLink} to={settingsPath}>
            <Settings size={15} aria-hidden="true" />
            Configure
          </Link>
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

      <dl className={styles.summary}>
        <div className={styles.summaryItem}>
          <dt>RPC endpoint</dt>
          <dd>
            <Tooltip content={urls.rpc}>
              <code>{urls.rpc}</code>
            </Tooltip>
            <CopyInlineButton
              value={urls.rpc}
              label="Copy RPC endpoint"
              copiedLabel="RPC endpoint copied"
              copiedChildren={null}
            >
              {null}
            </CopyInlineButton>
          </dd>
        </div>
      </dl>

      <div className={styles.setup}>
        <div className={styles.step}>
          <span className={styles.stepNumber}>1</span>
          <div className={styles.stepContent}>
            <h3>Choose what to connect</h3>
            <div className={styles.integrationOptions}>
              {integrationOptions.map(option => {
                const Icon = option.icon
                const selected = target === option.id

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
                      <small>{option.description}</small>
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
              <h3>{target === "rpc" ? "Use an endpoint" : "Add this setup"}</h3>
              {target === "acton" ? (
                <CopyInlineButton
                  value={actonConfig}
                  label="Copy Acton configuration"
                  copiedLabel="Acton configuration copied"
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
                <EndpointRow label="V2 API" value={urls.apiV2} />
                <EndpointRow label="V3 API" value={urls.apiV3} />
                <EndpointRow label="Control API" value={urls.control} />
              </div>
            ) : (
              <HighlightedCode
                className={styles.codeBlock}
                language={target === "acton" ? "toml" : "javascript"}
                value={target === "acton" ? actonConfig : tonClientSetup}
                ariaLabel={target === "acton" ? "Acton TOML configuration" : "JavaScript setup"}
              />
            )}
          </div>
        </div>

        {target === "acton" ? (
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
  environmentName,
  target,
  tonClientSetup,
  tonClientRequest,
  urls,
}: {
  readonly actonConfig: string
  readonly actonRunCommand: string
  readonly environmentName: string
  readonly target: IntegrationTarget
  readonly tonClientSetup: string
  readonly tonClientRequest: string
  readonly urls: {
    readonly apiV2: string
    readonly apiV3: string
    readonly control: string
    readonly explorer: string
    readonly rpc: string
  }
}): string {
  if (target === "acton") {
    return `Connect the Acton project to the "${environmentName}" virtual environment by adding this configuration to Acton.toml:

${actonConfig}

Then run a script on the localnet:

${actonRunCommand}`
  }

  if (target === "ton-client") {
    return `Connect the JavaScript application to the "${environmentName}" virtual environment with @ton/ton:

${tonClientSetup}

Make requests with the same TonClient API as testnet and mainnet:

${tonClientRequest}`
  }

  return `Connect the TON application to the "${environmentName}" Acton virtual environment.

RPC endpoint: ${urls.rpc}
V2 API: ${urls.apiV2}
V3 API: ${urls.apiV3}
Control API: ${urls.control}
Explorer: ${urls.explorer}`
}

function toAbsoluteUrl(value: string): string {
  try {
    return new URL(value, globalThis.location.origin).href
  } catch {
    return value
  }
}

function withoutTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value
}
