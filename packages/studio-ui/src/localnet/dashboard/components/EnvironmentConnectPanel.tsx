import {
  Button,
  CopyInlineAction,
  CopyInlineButton,
  HighlightedCode,
  InlineActions,
  Tooltip,
} from "@acton/ui"
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
    description: "Connect an @ton/ton client",
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
      explorer: toAbsoluteUrl(explorerUrl),
    }),
    [apiV2Url, apiV3Url, explorerUrl],
  )
  const endpointEntries = useMemo(() => {
    const entries: {label: string; value: string; href?: string}[] = []

    if (urls.apiV2) {
      entries.push({label: "TON Center v2", value: urls.apiV2})
    }

    if (urls.apiV3) {
      // Display and copy the API root because this is the value applications must configure.
      // Open the concrete docs file so an old browser-cached upstream redirect cannot bypass
      // the environment proxy. Studio still serves the same docs from the displayed API root.
      entries.push({
        label: "TON Center v3",
        value: urls.apiV3,
        href: `${withoutTrailingSlash(urls.apiV3)}/index.html`,
      })
    }

    return entries
  }, [urls.apiV2, urls.apiV3])
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
                    ? "Find the current TON Center endpoint"
                    : target === "acton" && !actonConfig
                      ? "Run a script on this network"
                      : "Add this setup"}
                </h3>
                {target === "rpc" ? (
                  <p className={styles.stepDescription}>
                    Search your app configuration for the Mainnet or Testnet TON Center URL it uses
                  </p>
                ) : target === "acton" && !actonConfig ? (
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

            {target === "rpc" ? undefined : target === "acton" && !actonConfig ? (
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

        {target === "rpc" ? (
          <div className={styles.step}>
            <span className={styles.stepNumber}>3</span>
            <div className={styles.stepContent}>
              <div>
                <h3>Point your app to this environment</h3>
                <p className={styles.stepDescription}>
                  Use the endpoint that matches your app&apos;s TON Center API version
                </p>
              </div>
              <dl className={styles.endpointList} aria-label="RPC endpoints">
                {endpointEntries.map(endpoint => (
                  <EndpointRow key={endpoint.label} {...endpoint} />
                ))}
              </dl>
            </div>
          </div>
        ) : undefined}

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

function EndpointRow({
  label,
  value,
  href = value,
}: {
  readonly label: string
  readonly value: string
  readonly href?: string
}) {
  return (
    <div className={styles.endpointRow}>
      <dt className={styles.endpointLabel}>{label}</dt>
      <dd className={styles.endpointValue}>
        {/* Opening and copying are separate actions so copying never triggers navigation. */}
        <InlineActions
          className={styles.endpointActions}
          visibility="always"
          actions={
            <CopyInlineAction
              value={value}
              label={`Copy ${label} endpoint`}
              copiedLabel={`${label} endpoint copied`}
              size="compact"
            />
          }
        >
          <Tooltip content={value}>
            <a
              className={styles.endpointLink}
              href={href}
              target="_blank"
              rel="noreferrer"
              aria-label={`Open ${label} endpoint`}
            >
              <code>{value}</code>
            </a>
          </Tooltip>
        </InlineActions>
      </dd>
    </div>
  )
}

/**
 * Builds a self-contained prompt for a coding agent. The prompt includes exact connection values
 * and guards against duplicate clients, configuration sections, and unrelated project changes.
 */
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
    readonly explorer?: string
  }
}): string {
  if (target === "acton") {
    if (!actonConfig) {
      return `Connect this Acton project to the Acton Studio environment "${environmentName}"

Requirements:
- Inspect Acton.toml and the scripts directory before you change files
- Use the existing project structure and commands
- Do not add a custom network entry because Studio routes Acton's built-in ${actonNetworkName} network while this environment is running
- Keep unrelated configuration and code unchanged

Run an existing script with this command:

\`\`\`shell
${actonRunCommand}
\`\`\`

If scripts/deploy.tolk does not exist, replace that path with the relevant existing script

Validation:
- Run the selected script while Acton Studio and this environment are running
- Report the files that you changed and the validation result`
    }

    return `Connect this Acton project to the Acton Studio environment "${environmentName}"

Requirements:
- Inspect Acton.toml and the scripts directory before you change files
- Add or update the network section with the exact configuration below
- If the network section exists, update it instead of adding a duplicate section
- Keep unrelated configuration and code unchanged

Acton.toml configuration:

\`\`\`toml
${actonConfig}
\`\`\`

Run an existing script with this command:

\`\`\`shell
${actonRunCommand}
\`\`\`

If scripts/deploy.tolk does not exist, replace that path with the relevant existing script

Validation:
- Run the selected script while Acton Studio and this environment are running
- Report the files that you changed and the validation result`
  }

  if (target === "ton-client") {
    return `Connect this JavaScript or TypeScript application to the Acton Studio environment "${environmentName}" with @ton/ton

Requirements:
- Inspect the package manager, existing TonClient creation, and endpoint configuration before you change files
- Reuse the existing TonClient and configuration path when they exist
- Do not create a second client or replace the project's package manager
- If @ton/ton is missing, add it with the package manager that the project already uses
- Keep existing client options and unrelated code unchanged

TON Center v2 JSON-RPC endpoint: ${withoutTrailingSlash(urls.apiV2 ?? "")}/jsonRPC

Use this setup only if the application has no TonClient:

\`\`\`typescript
${tonClientSetup}
\`\`\`

Use this safe read request to validate the connection:

\`\`\`typescript
${tonClientRequest}
\`\`\`

The endpoint is available while Acton Studio and this environment are running

Validation:
- Run the project's existing typecheck and relevant tests
- Run the read request against this environment
- Report the files that you changed and the validation result`
  }

  const v2JsonRpc = urls.apiV2 ? `${withoutTrailingSlash(urls.apiV2)}/jsonRPC` : undefined
  const endpointLines = [
    urls.apiV2 ? `TON Center v2: ${urls.apiV2}` : undefined,
    v2JsonRpc ? `TON Center v2 JSON-RPC: ${v2JsonRpc}` : undefined,
    urls.apiV3 ? `TON Center v3: ${urls.apiV3}` : undefined,
    urls.explorer ? `Explorer: ${urls.explorer}` : undefined,
  ].filter((line): line is string => Boolean(line))

  return `Connect this TON application to the Acton Studio environment "${environmentName}"

Requirements:
- Search the configuration, .env files, and source code for current Mainnet or Testnet TON Center URLs
- Identify the API version for each client from its current endpoint and request format
- Replace each public endpoint base with the matching Acton Studio endpoint below
- Keep existing request paths, HTTP methods, payloads, client options, and unrelated configuration unchanged
- Do not convert TON Center v2 calls to v3 or v3 calls to v2

Connection values:

${endpointLines.map(line => `- ${line}`).join("\n")}

Use the TON Center v2 JSON-RPC URL only for clients that require the complete JSON-RPC endpoint
These endpoints are available while Acton Studio and this environment are running

Validation:
- Search again for public TON Center URLs and review every remaining match
- Run the project's existing checks and one safe read request through the configured client
- Report the files that you changed, the endpoints that you replaced, and the validation result`
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
