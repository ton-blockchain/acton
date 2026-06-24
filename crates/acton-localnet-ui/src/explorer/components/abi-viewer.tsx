import {Buffer} from "node:buffer"
import {useEffect, useMemo, useState} from "react"
import type {JSX, MouseEvent, ReactNode} from "react"

import {DataBlock, jetbrainsDarculaTheme, jetbrainsLightTheme} from "@acton/shared-ui"
import {
  Address,
  Cell,
  Dictionary,
  TupleReader,
  type ContractProvider,
  type TupleItem,
} from "@ton/core"
import {
  callGetMethodDynamic,
  DynamicCtx,
  renderTy,
  type ABIGetMethod,
  type ContractABI,
  type SymTable,
  type Ty,
} from "@ton/tolk-abi-to-typescript"
import {Link2, Play} from "lucide-react"
import {createHighlighterCore} from "shiki/core"
import {createOnigurumaEngine} from "shiki/engine/oniguruma"
import type {LanguageRegistration} from "shiki/types"

import type {TonClient} from "../api/client"
import type {V3RunGetMethodResponse, V3RunGetMethodStackEntry} from "../api/types"

import tolkGrammarRaw from "../../../../../docs/grammars/grammar-tolk.json"

import {abiSymbolAnchorId} from "./abiAnchors"
import styles from "./abi-viewer.module.css"

export type AbiTab = "view" | "raw"
type HighlightLanguage = "json" | "tolk"
type AbiSimpleArgKind = "number" | "string" | "bool" | "address" | "cell"

interface AbiSimpleArgInput {
  readonly kind: AbiSimpleArgKind
  readonly nullable: boolean
}

interface StructFieldInfo {
  readonly tyIdx: number
  readonly description?: string
}

interface CellLike {
  readonly asCell: () => Cell
}

interface AbiPanelProps {
  readonly activeTab: AbiTab
  readonly onTabChange: (tab: AbiTab) => void
  readonly abi: ContractABI
  readonly ownerAddress?: string
  readonly client?: TonClient
  readonly getMethodsMode?: "interactive" | "readonly"
  readonly heightMode?: "contained" | "content"
  readonly showSymbolAnchors?: boolean
}

type AbiDeclaration = Readonly<ContractABI["declarations"][number]>
type AbiEnumMemberWithDescription = Readonly<{
  readonly description?: string
}>
type AbiMessage = Readonly<
  | ContractABI["incoming_messages"][number]
  | ContractABI["incoming_external"][number]
  | ContractABI["outgoing_messages"][number]
  | ContractABI["emitted_events"][number]
>

const grammarWithName = (grammar: unknown, name: string): LanguageRegistration =>
  ({
    ...(grammar as Record<string, unknown>),
    name,
  }) as LanguageRegistration

const tolkGrammar = grammarWithName(tolkGrammarRaw, "tolk")

let abiHighlighterPromise: ReturnType<typeof createHighlighterCore> | undefined

const getAbiHighlighter = () => {
  abiHighlighterPromise ??= createHighlighterCore({
    themes: [jetbrainsLightTheme, jetbrainsDarculaTheme],
    langs: [tolkGrammar, import("shiki/langs/json.mjs")],
    engine: createOnigurumaEngine(() => import("shiki/wasm")),
  })

  return abiHighlighterPromise
}

export function AbiPanel({
  activeTab,
  onTabChange,
  abi,
  ownerAddress,
  client,
  getMethodsMode = ownerAddress && client ? "interactive" : "readonly",
  heightMode = "contained",
  showSymbolAnchors = false,
}: AbiPanelProps): JSX.Element {
  const abiJson = useMemo(() => JSON.stringify(abi, undefined, 2), [abi])
  const abiTabs: readonly {tab: AbiTab; label: string}[] = [
    {tab: "view", label: "Rendered"},
    {tab: "raw", label: "Raw JSON"},
  ]
  const rootClassName = [
    styles.sourceShell,
    heightMode === "content" ? styles.sourceShellContent : "",
  ]
    .filter(Boolean)
    .join(" ")

  useEffect(() => {
    const scrollToCurrentHashSymbol = () => {
      if (!globalThis.location.hash) {
        return
      }

      const id = decodeURIComponent(globalThis.location.hash.slice(1))
      if (!id.startsWith("abi-")) {
        return
      }

      const target = globalThis.document.getElementById(id)
      if (target instanceof HTMLDetailsElement) {
        target.open = true
      }
      scrollToAbiSymbol(target)
    }
    const openCurrentHashSymbol = () => {
      scrollToCurrentHashSymbol()
      globalThis.requestAnimationFrame(scrollToCurrentHashSymbol)
      globalThis.setTimeout(scrollToCurrentHashSymbol, 80)
    }

    openCurrentHashSymbol()
    globalThis.addEventListener("hashchange", openCurrentHashSymbol)
    return () => globalThis.removeEventListener("hashchange", openCurrentHashSymbol)
  }, [abi])

  return (
    <section className={rootClassName}>
      <div className={styles.editorTabBar}>
        {abiTabs.map(item => (
          <button
            key={item.tab}
            type="button"
            className={`${styles.editorTab} ${activeTab === item.tab ? styles.editorTabActive : ""}`}
            onClick={() => onTabChange(item.tab)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {activeTab === "view" ? (
        <AbiViewPanel
          abi={abi}
          ownerAddress={ownerAddress}
          client={client}
          getMethodsMode={getMethodsMode}
          showSymbolAnchors={showSymbolAnchors}
        />
      ) : (
        <DataBlock
          className={styles.sourceDataBlock}
          contentClassName={heightMode === "content" ? styles.sourceDataBlockContent : undefined}
          variant="standalone"
          copyLabel="ABI"
          copyValue={abiJson}
        >
          <CodeContent value={abiJson} language="json" wrap={false} />
        </DataBlock>
      )}
    </section>
  )
}

function AbiViewPanel({
  abi,
  ownerAddress,
  client,
  getMethodsMode,
  showSymbolAnchors,
}: {
  readonly abi: ContractABI
  readonly ownerAddress?: string
  readonly client?: TonClient
  readonly getMethodsMode: "interactive" | "readonly"
  readonly showSymbolAnchors: boolean
}): JSX.Element {
  const ctx = useMemo(() => new DynamicCtx(abi), [abi])
  const symbols = ctx.symbols
  const canRunGetMethods =
    getMethodsMode === "interactive" && ownerAddress !== undefined && client !== undefined

  return (
    <section className={`${styles.dataPanel} ${styles.abiPanel}`}>
      <div className={styles.abiView}>
        <header className={styles.abiHeader}>
          <div className={styles.abiHeaderMain}>
            <h3 className={styles.abiTitle}>{abi.contract_name}</h3>
            <div className={styles.abiFacts} aria-label="Compiler ABI metadata">
              <span>
                <strong>Compiler:</strong> {abi.compiler_name} {abi.compiler_version}
              </span>
              {abi.version && (
                <span>
                  <strong>Contract version:</strong> v{abi.version}
                </span>
              )}
              {abi.author && (
                <span>
                  <strong>Author:</strong> {abi.author}
                </span>
              )}
            </div>
            {abi.description && <p className={styles.abiDescription}>{abi.description}</p>}
          </div>
        </header>

        <AbiGetMethodsSection
          methods={abi.get_methods}
          ctx={ctx}
          ownerAddress={canRunGetMethods ? ownerAddress : undefined}
          client={canRunGetMethods ? client : undefined}
          showSymbolAnchors={showSymbolAnchors}
        />
        <AbiMessagesSection abi={abi} symbols={symbols} showSymbolAnchors={showSymbolAnchors} />
        <AbiStorageSection
          storage={abi.storage}
          symbols={symbols}
          showSymbolAnchors={showSymbolAnchors}
        />
        <AbiDeclarationsSection
          declarations={abi.declarations}
          symbols={symbols}
          showSymbolAnchors={showSymbolAnchors}
        />
        <AbiThrownErrorsSection errors={abi.thrown_errors} showSymbolAnchors={showSymbolAnchors} />
      </div>
    </section>
  )
}

function AbiGetMethodsSection({
  methods,
  ctx,
  ownerAddress,
  client,
  showSymbolAnchors,
}: {
  readonly methods: readonly ABIGetMethod[]
  readonly ctx: DynamicCtx
  readonly ownerAddress?: string
  readonly client?: TonClient
  readonly showSymbolAnchors: boolean
}): JSX.Element {
  const canRun = ownerAddress !== undefined && client !== undefined

  return (
    <AbiSection title="Get methods" count={methods.length}>
      {methods.length > 0 ? (
        <div className={styles.abiMethodList}>
          {methods.map(method => (
            <AbiGetMethodItem
              key={`${method.name}:${method.tvm_method_id}`}
              method={method}
              ctx={ctx}
              ownerAddress={canRun ? ownerAddress : undefined}
              client={canRun ? client : undefined}
              showSymbolAnchors={showSymbolAnchors}
            />
          ))}
        </div>
      ) : (
        <div className={styles.abiEmptyInline}>No get methods declared</div>
      )}
    </AbiSection>
  )
}

function AbiGetMethodItem({
  method,
  ctx,
  ownerAddress,
  client,
  showSymbolAnchors,
}: {
  readonly method: ABIGetMethod
  readonly ctx: DynamicCtx
  readonly ownerAddress?: string
  readonly client?: TonClient
  readonly showSymbolAnchors: boolean
}): JSX.Element {
  const [argsJson, setArgsJson] = useState("[]")
  const [argValues, setArgValues] = useState<readonly string[]>(
    method.parameters.map(parameter => initialSimpleArgValue(ctx.symbols, parameter.ty_idx)),
  )
  const [jsonError, setJsonError] = useState<string | undefined>()
  const [runState, setRunState] = useState<
    | {readonly status: "idle"}
    | {readonly status: "loading"}
    | {
        readonly status: "success"
        readonly result: V3RunGetMethodResponse
        readonly decoded: unknown
      }
    | {readonly status: "error"; readonly error: string}
  >({status: "idle"})
  const hasParameters = method.parameters.length > 0
  const argsInputId = `args-${method.name}`
  const symbols = ctx.symbols
  const simpleArgInputs = method.parameters.map(parameter =>
    getSimpleArgInput(symbols, parameter.ty_idx),
  )
  const canRenderSimpleArgs = hasParameters && simpleArgInputs.every(Boolean)
  const canRun = ownerAddress !== undefined && client !== undefined
  const methodId = abiSymbolAnchorId("get-method", method.name)

  const runMethod = async () => {
    if (!canRun) return

    let args: readonly unknown[] = []
    if (hasParameters) {
      try {
        args = canRenderSimpleArgs
          ? parseSimpleGetMethodArgs(ctx, method, argValues)
          : parseGetMethodArgs(ctx, method, argsJson)
      } catch (error) {
        setJsonError(error instanceof Error ? error.message : String(error))
        return
      }
    }

    setJsonError(undefined)
    setRunState({status: "loading"})
    try {
      let result: V3RunGetMethodResponse | undefined
      const provider = createGetMethodProvider(client, ownerAddress, value => {
        result = value
      })
      const decoded: unknown = await callGetMethodDynamic(provider, ctx, method.name, [...args])
      if (!result) {
        throw new Error("Get method response was not captured.")
      }
      setRunState({status: "success", result, decoded})
    } catch (error) {
      setRunState({
        status: "error",
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }

  return (
    <article id={methodId} className={styles.abiMethod}>
      <div className={styles.abiMethodTopline}>
        <div className={styles.abiSignatureBlock}>
          <div className={styles.abiSignatureLine}>
            <TolkCode value={formatGetMethodSignature(method, symbols)} />
            <sup className={styles.abiMethodId}>method id: {method.tvm_method_id}</sup>
            <AbiSymbolAnchor
              show={showSymbolAnchors}
              id={methodId}
              label={`Link to ${method.name}`}
            />
          </div>
        </div>
        {canRun && (
          <button
            type="button"
            className={styles.abiRunButton}
            onClick={() => {
              void runMethod()
            }}
            disabled={runState.status === "loading"}
          >
            <Play size={14} aria-hidden="true" />
            {runState.status === "loading" ? "Running" : "Run"}
          </button>
        )}
      </div>

      {method.description && <p className={styles.abiMethodDescription}>{method.description}</p>}

      {canRun && canRenderSimpleArgs ? (
        <>
          <div className={styles.abiArgsGrid}>
            {method.parameters.map((parameter, index) => {
              const input = simpleArgInputs[index]
              if (!input) return
              const inputId = `${argsInputId}-${parameter.name}-${index}`

              return (
                <label
                  className={styles.abiArgField}
                  key={`${parameter.name}-${index}`}
                  htmlFor={inputId}
                >
                  <span className={styles.abiArgLabel}>
                    <strong>{formatTolkIdentifier(parameter.name)}</strong>
                    <code>{formatType(symbols, parameter.ty_idx)}</code>
                  </span>
                  {input.kind === "bool" ? (
                    <select
                      id={inputId}
                      className={styles.abiArgInput}
                      value={argValues[index] ?? "false"}
                      onChange={event => {
                        setArgValues(values =>
                          values.map((value, valueIndex) =>
                            valueIndex === index ? event.target.value : value,
                          ),
                        )
                        setJsonError(undefined)
                      }}
                    >
                      {input.nullable && <option value="">null</option>}
                      <option value="false">false</option>
                      <option value="true">true</option>
                    </select>
                  ) : (
                    <input
                      id={inputId}
                      className={styles.abiArgInput}
                      value={argValues[index] ?? ""}
                      placeholder={placeholderForSimpleArg(input)}
                      inputMode={input.kind === "number" ? "decimal" : "text"}
                      spellCheck={false}
                      onChange={event => {
                        setArgValues(values =>
                          values.map((value, valueIndex) =>
                            valueIndex === index ? event.target.value : value,
                          ),
                        )
                        setJsonError(undefined)
                      }}
                    />
                  )}
                </label>
              )
            })}
          </div>
          {jsonError && <div className={styles.abiError}>{jsonError}</div>}
        </>
      ) : canRun && hasParameters ? (
        <div className={styles.abiStackInputBlock}>
          <label className={styles.abiStackLabel} htmlFor={argsInputId}>
            Arguments JSON
          </label>
          <textarea
            id={argsInputId}
            className={styles.abiStackInput}
            value={argsJson}
            placeholder={formatArgsPlaceholder(method, symbols)}
            spellCheck={false}
            onChange={event => {
              setArgsJson(event.target.value)
              setJsonError(undefined)
            }}
          />
          {jsonError && <div className={styles.abiError}>{jsonError}</div>}
        </div>
      ) : undefined}

      {runState.status === "loading" && <AbiGetMethodSkeleton />}
      {runState.status === "error" && <div className={styles.abiError}>{runState.error}</div>}
      {runState.status === "success" && (
        <AbiGetMethodResult
          result={runState.result}
          decoded={runState.decoded}
          method={method}
          symbols={symbols}
        />
      )}
    </article>
  )
}

function AbiGetMethodSkeleton(): JSX.Element {
  return (
    <div className={styles.abiResultSkeleton} aria-label="Running get method">
      <span />
      <span />
      <span />
    </div>
  )
}

function AbiGetMethodResult({
  result,
  decoded,
  method,
  symbols,
}: {
  readonly result: V3RunGetMethodResponse
  readonly decoded: unknown
  readonly method: ABIGetMethod
  readonly symbols: SymTable
}): JSX.Element {
  return (
    <div className={styles.abiResult}>
      <AbiDecodedResult decoded={decoded} method={method} symbols={symbols} />
      <div className={styles.abiResultStats}>
        <span>
          <strong>Exit code:</strong> {result.exit_code}
        </span>
        <span>
          <strong>Gas used:</strong> {result.gas_used}
        </span>
      </div>
      <details className={styles.abiDetails}>
        <summary>Raw stack JSON</summary>
        <div className={styles.abiDetailsCode}>
          <CodeContent
            value={JSON.stringify(result.stack, undefined, 2)}
            language="json"
            wrap={false}
          />
        </div>
      </details>
      {result.vm_log && (
        <details className={styles.abiDetails}>
          <summary>VM log</summary>
          <pre>{result.vm_log}</pre>
        </details>
      )}
    </div>
  )
}

function AbiDecodedResult({
  decoded,
  method,
  symbols,
}: {
  readonly decoded: unknown
  readonly method: ABIGetMethod
  readonly symbols: SymTable
}): JSX.Element {
  const displayValue = decodedDisplayValue(decoded)
  if (isPlainDecodedValue(displayValue)) {
    return (
      <div className={styles.abiDecodedResult}>
        <span>{String(displayValue)}</span>
      </div>
    )
  }

  return (
    <div className={styles.abiDecodedResult}>
      <TolkCode value={formatDecodedTolkValue(displayValue, symbols, method.return_ty_idx)} />
    </div>
  )
}

function AbiMessagesSection({
  abi,
  symbols,
  showSymbolAnchors,
}: {
  readonly abi: ContractABI
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}): JSX.Element {
  const groups: readonly {
    readonly title: string
    readonly messages: readonly AbiMessage[]
    readonly empty: string
  }[] = [
    {
      title: "Incoming/internal",
      messages: abi.incoming_messages,
      empty: "No incoming internal messages declared",
    },
    {
      title: "Incoming external",
      messages: abi.incoming_external,
      empty: "No incoming external messages declared",
    },
    {
      title: "Outgoing",
      messages: abi.outgoing_messages,
      empty: "No outgoing messages declared",
    },
    {
      title: "Emitted events",
      messages: abi.emitted_events,
      empty: "No emitted events declared",
    },
  ]
  const count = groups.reduce((total, group) => total + group.messages.length, 0)

  return (
    <AbiSection title="Messages" count={count}>
      <div className={styles.abiMessageGrid}>
        {groups.map(group => (
          <section key={group.title} className={styles.abiMessageGroup}>
            <header className={styles.abiMessageGroupHeader}>
              <span>{group.title}</span>
              <span className={styles.abiCount}>{group.messages.length}</span>
            </header>
            {group.messages.length > 0 ? (
              group.messages.map(message => (
                <AbiMessageRow
                  key={`${group.title}:${message.body_ty_idx}`}
                  groupTitle={group.title}
                  message={message}
                  symbols={symbols}
                  showSymbolAnchors={showSymbolAnchors}
                />
              ))
            ) : (
              <div className={styles.abiEmptyInline}>{group.empty}</div>
            )}
          </section>
        ))}
      </div>
    </AbiSection>
  )
}

function AbiMessageRow({
  groupTitle,
  message,
  symbols,
  showSymbolAnchors,
}: {
  readonly groupTitle: string
  readonly message: AbiMessage
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}): JSX.Element {
  const declaration = getAbiTyDeclaration(symbols, message.body_ty_idx)
  const messageName = declaration?.name ?? `type-${message.body_ty_idx}`
  const messageId = abiSymbolAnchorId("message", `${groupTitle}-${messageName}`)

  return (
    <div id={messageId} className={styles.abiMessageRow}>
      <div className={styles.abiSymbolLine}>
        <TolkCode value={formatAbiTyDeclaration(symbols, message.body_ty_idx)} />
        <AbiSymbolAnchor show={showSymbolAnchors} id={messageId} label={`Link to ${messageName}`} />
      </div>
      {declaration?.description && (
        <p className={styles.abiDeclarationDescription}>{declaration.description}</p>
      )}
    </div>
  )
}

function AbiStorageSection({
  storage,
  symbols,
  showSymbolAnchors,
}: {
  readonly storage: ContractABI["storage"]
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}): JSX.Element {
  const rows = [
    {label: "storage", tyIdx: storage.storage_ty_idx},
    {
      label: "storageAtDeployment",
      tyIdx: storage.storage_at_deployment_ty_idx,
    },
  ].filter((row): row is {label: string; tyIdx: number} => row.tyIdx !== undefined)
  const showStorageLabels = rows.length > 1

  return (
    <AbiSection title="Storage" count={rows.length}>
      {rows.length > 0 ? (
        <div className={styles.abiRows}>
          {rows.map(row => {
            const declaration = getAbiTyDeclaration(symbols, row.tyIdx)
            const storageId = abiSymbolAnchorId("storage", row.label)
            const showHeader = showStorageLabels || showSymbolAnchors

            return (
              <div id={storageId} key={row.label} className={styles.abiRow}>
                {showHeader && (
                  <div
                    className={
                      showStorageLabels ? styles.abiStorageHeader : styles.abiStorageAnchorRow
                    }
                  >
                    {showStorageLabels && (
                      <span className={styles.abiStorageName}>{row.label}</span>
                    )}
                    <AbiSymbolAnchor
                      show={showSymbolAnchors}
                      id={storageId}
                      label={`Link to ${row.label}`}
                    />
                  </div>
                )}
                <TolkCode value={formatAbiTyDeclaration(symbols, row.tyIdx)} />
                {declaration?.description && (
                  <p className={styles.abiDeclarationDescription}>{declaration.description}</p>
                )}
              </div>
            )
          })}
        </div>
      ) : (
        <div className={styles.abiEmptyInline}>No storage type indexes declared</div>
      )}
    </AbiSection>
  )
}

function AbiDeclarationsSection({
  declarations,
  symbols,
  showSymbolAnchors,
}: {
  readonly declarations: readonly AbiDeclaration[]
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}): JSX.Element {
  return (
    <AbiSection title="Declarations" count={declarations.length}>
      {declarations.length > 0 ? (
        <div className={styles.abiDeclarationList}>
          {declarations.map(declaration => {
            const declarationId = abiSymbolAnchorId("declaration", declaration.name)

            return (
              <details
                id={declarationId}
                key={`${declaration.kind}:${declaration.name}:${declaration.ty_idx}`}
                className={styles.abiDeclaration}
              >
                <summary>
                  <span className={styles.abiDeclarationName}>{declaration.name}</span>
                  <sup
                    className={`${styles.abiDeclarationKind} ${declarationKindClass(
                      declaration.kind,
                    )}`}
                  >
                    {declaration.kind}
                  </sup>
                  <AbiSymbolAnchor
                    show={showSymbolAnchors}
                    id={declarationId}
                    label={`Link to ${declaration.name}`}
                    onClick={event => {
                      event.stopPropagation()
                      const details = event.currentTarget.closest("details")
                      if (details instanceof HTMLDetailsElement) {
                        details.open = true
                      }
                    }}
                  />
                </summary>
                {renderDeclarationBody(declaration, symbols)}
              </details>
            )
          })}
        </div>
      ) : (
        <div className={styles.abiEmptyInline}>No declarations emitted</div>
      )}
    </AbiSection>
  )
}

function AbiThrownErrorsSection({
  errors,
  showSymbolAnchors,
}: {
  readonly errors: readonly ContractABI["thrown_errors"][number][]
  readonly showSymbolAnchors: boolean
}): JSX.Element {
  return (
    <AbiSection title="Thrown errors" count={errors.length}>
      {errors.length > 0 ? (
        <div className={styles.abiRows}>
          {errors.map(error => {
            const errorName = error.name ?? String(error.err_code)
            const errorId = abiSymbolAnchorId("error", errorName, String(error.err_code))

            return (
              <div
                id={errorId}
                key={`${error.err_code}:${error.name ?? error.kind}`}
                className={`${styles.abiErrorRow} ${
                  showSymbolAnchors ? "" : styles.abiErrorRowNoAnchor
                }`}
              >
                <span className={styles.abiErrorCode}>{error.err_code}</span>
                <span className={styles.abiErrorName}>{errorName}</span>
                <span className={styles.abiMuted}>{error.description ?? ""}</span>
                <AbiSymbolAnchor
                  show={showSymbolAnchors}
                  id={errorId}
                  label={`Link to ${errorName}`}
                />
              </div>
            )
          })}
        </div>
      ) : (
        <div className={styles.abiEmptyInline}>No thrown errors declared</div>
      )}
    </AbiSection>
  )
}

function AbiSymbolAnchor({
  show,
  id,
  label,
  onClick,
}: {
  readonly show: boolean
  readonly id: string
  readonly label: string
  readonly onClick?: (event: MouseEvent<HTMLAnchorElement>) => void
}): JSX.Element | null {
  if (!show) {
    return null
  }

  return (
    <a className={styles.abiSymbolAnchor} href={`#${id}`} aria-label={label} onClick={onClick}>
      <Link2 size={12} aria-hidden="true" />
    </a>
  )
}

function AbiSection({
  title,
  count,
  children,
}: {
  readonly title: string
  readonly count: number
  readonly children: ReactNode
}): JSX.Element {
  return (
    <section className={styles.abiSection}>
      <header className={styles.abiSectionHeader}>
        <h4>{title}</h4>
        <span className={styles.abiCount}>{count}</span>
      </header>
      {children}
    </section>
  )
}

function parseSimpleGetMethodArgs(
  ctx: DynamicCtx,
  method: ABIGetMethod,
  values: readonly string[],
): unknown[] {
  if (values.length !== method.parameters.length) {
    throw new Error(`Expected ${method.parameters.length} arguments, got ${values.length}.`)
  }

  return method.parameters.map((parameter, index) =>
    normalizeSimpleDynamicArg(ctx, parameter.ty_idx, values[index] ?? ""),
  )
}

function parseGetMethodArgs(ctx: DynamicCtx, method: ABIGetMethod, value: string): unknown[] {
  const parsed = JSON.parse(value.trim() || "[]") as unknown
  if (!Array.isArray(parsed)) {
    throw new TypeError("Arguments must be a JSON array.")
  }
  if (parsed.length !== method.parameters.length) {
    throw new Error(`Expected ${method.parameters.length} arguments, got ${parsed.length}.`)
  }

  return method.parameters.map((parameter, index) =>
    normalizeDynamicArg(ctx, parameter.ty_idx, parsed[index]),
  )
}

function normalizeSimpleDynamicArg(ctx: DynamicCtx, tyIdx: number, value: string): unknown {
  const ty = ctx.symbols.tyByIdx(tyIdx)
  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return BigInt(requireArgValue(value, "Number argument"))
    }
    case "bool": {
      if (value === "true") return true
      if (value === "false") return false
      throw new Error("Boolean argument must be true or false.")
    }
    case "string": {
      return value
    }
    case "address":
    case "addressExt": {
      return Address.parse(requireArgValue(value, "Address argument"))
    }
    case "addressOpt":
    case "addressAny": {
      return value.trim() ? Address.parse(value.trim()) : undefined
    }
    case "cell": {
      return parseCellArg(value)
    }
    case "builder": {
      return parseCellArg(value).asBuilder()
    }
    case "slice":
    case "remaining":
    case "bitsN": {
      return parseCellArg(value).beginParse()
    }
    case "nullable": {
      return value.trim() ? normalizeSimpleDynamicArg(ctx, ty.inner_ty_idx, value) : undefined
    }
    case "AliasRef": {
      const target = ctx.symbols.aliasTargetOf(tyIdx)
      return normalizeSimpleDynamicArg(ctx, target.ty_idx, value)
    }
    default: {
      return normalizeDynamicArg(ctx, tyIdx, value)
    }
  }
}

function normalizeDynamicArg(ctx: DynamicCtx, tyIdx: number, value: unknown): unknown {
  const ty = ctx.symbols.tyByIdx(tyIdx)
  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return typeof value === "string" ? BigInt(value) : value
    }
    case "address":
    case "addressExt": {
      return typeof value === "string" ? Address.parse(value) : value
    }
    case "addressOpt": {
      return typeof value === "string" ? Address.parse(value) : value
    }
    case "addressAny": {
      return typeof value === "string" && value !== "none" ? Address.parse(value) : value
    }
    case "cell": {
      return typeof value === "string" ? Cell.fromBase64(value) : value
    }
    case "builder": {
      return typeof value === "string" ? Cell.fromBase64(value).asBuilder() : value
    }
    case "slice":
    case "remaining":
    case "bitsN": {
      return typeof value === "string" ? Cell.fromBase64(value).beginParse() : value
    }
    case "cellOf": {
      if (isRecord(value) && "ref" in value) {
        return {ref: normalizeDynamicArg(ctx, ty.inner_ty_idx, value.ref)}
      }
      return value
    }
    case "nullable": {
      return value == undefined ? undefined : normalizeDynamicArg(ctx, ty.inner_ty_idx, value)
    }
    case "arrayOf":
    case "lispListOf": {
      return Array.isArray(value)
        ? value.map(item => normalizeDynamicArg(ctx, ty.inner_ty_idx, item))
        : value
    }
    case "tensor":
    case "shapedTuple": {
      return Array.isArray(value)
        ? value.map((item, index) => normalizeDynamicArg(ctx, ty.items_ty_idx[index], item))
        : value
    }
    case "mapKV": {
      if (isRecord(value)) {
        return new Map(
          Object.entries(value).map(([key, item]) => [
            normalizeDynamicMapKey(ctx, ty.key_ty_idx, key),
            normalizeDynamicArg(ctx, ty.value_ty_idx, item),
          ]),
        )
      }
      return value
    }
    case "StructRef": {
      if (isRecord(value)) {
        return Object.fromEntries(
          ctx.symbols
            .structFieldsOf(tyIdx, true)
            .map(field => [field.name, normalizeDynamicArg(ctx, field.ty_idx, value[field.name])]),
        )
      }
      return value
    }
    case "AliasRef": {
      const target = ctx.symbols.aliasTargetOf(tyIdx)
      return normalizeDynamicArg(ctx, target.ty_idx, value)
    }
    default: {
      return value
    }
  }
}

function requireArgValue(value: string, label: string): string {
  const trimmed = value.trim()
  if (!trimmed) {
    throw new Error(`${label} is required.`)
  }
  return trimmed
}

function parseCellArg(value: string): Cell {
  const trimmed = requireArgValue(value, "Cell argument")
  const hex = trimmed.startsWith("0x") ? trimmed.slice(2) : trimmed
  if (/^(?:[0-9a-fA-F]{2})+$/.test(hex)) {
    return Cell.fromBoc(Buffer.from(hex, "hex"))[0]
  }
  return Cell.fromBase64(trimmed)
}

function normalizeDynamicMapKey(ctx: DynamicCtx, tyIdx: number, value: string): unknown {
  const ty = ctx.symbols.tyByIdx(tyIdx)
  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return BigInt(value)
    }
    case "address":
    case "addressOpt":
    case "addressAny": {
      return Address.parse(value)
    }
    case "AliasRef": {
      const target = ctx.symbols.aliasTargetOf(tyIdx)
      return normalizeDynamicMapKey(ctx, target.ty_idx, value)
    }
    default: {
      return value
    }
  }
}

function tupleItemToV3StackEntry(item: TupleItem): V3RunGetMethodStackEntry {
  switch (item.type) {
    case "int": {
      return {type: "num", value: item.value.toString()}
    }
    case "null": {
      return {type: "null", value: undefined}
    }
    case "cell":
    case "slice":
    case "builder": {
      return {type: item.type, value: item.cell.toBoc().toString("base64")}
    }
    case "tuple": {
      return {
        type: "tuple",
        value: item.items.map(value => tupleItemToV3StackEntry(value)),
      }
    }
    case "nan": {
      throw new Error("NaN tuple items cannot be passed to runGetMethod.")
    }
  }
}

function v3StackEntryToTupleItem(entry: V3RunGetMethodStackEntry): TupleItem {
  switch (entry.type) {
    case "num": {
      return {type: "int", value: BigInt(String(entry.value))}
    }
    case "null": {
      return {type: "null"}
    }
    case "cell":
    case "slice":
    case "builder": {
      return {
        type: entry.type,
        cell: Cell.fromBase64(extractStackBoc(entry.value, entry.type)),
      }
    }
    case "tuple":
    case "list": {
      if (!Array.isArray(entry.value)) {
        throw new TypeError(`${entry.type} stack value must be an array.`)
      }
      return {
        type: "tuple",
        items: entry.value
          .map(value => assertV3StackEntry(value))
          .map(value => v3StackEntryToTupleItem(value)),
      }
    }
    case "nan": {
      return {type: "nan"}
    }
    default: {
      throw new Error(`Unsupported stack entry type: ${entry.type}.`)
    }
  }
}

function assertV3StackEntry(value: unknown): V3RunGetMethodStackEntry {
  if (!isRecord(value) || typeof value.type !== "string") {
    throw new Error("Nested stack entry must include string `type`.")
  }
  return {
    type: value.type,
    value: Object.prototype.hasOwnProperty.call(value, "value") ? value.value : undefined,
  }
}

function extractStackBoc(value: unknown, type: string): string {
  if (typeof value === "string") {
    return value
  }
  if (isRecord(value) && typeof value.bytes === "string") {
    return value.bytes
  }
  throw new Error(`${type} stack value must be a base64 string or {bytes}.`)
}

function createGetMethodProvider(
  client: TonClient,
  ownerAddress: string,
  onResult: (result: V3RunGetMethodResponse) => void,
): ContractProvider {
  return {
    async get(name, args) {
      const result = await client.runGetMethod(
        ownerAddress,
        name,
        args.map(value => tupleItemToV3StackEntry(value)),
      )
      onResult(result)
      return {
        stack: new TupleReader(result.stack.map(value => v3StackEntryToTupleItem(value))),
        gasUsed: BigInt(result.gas_used),
        logs: result.vm_log,
      }
    },
  } as ContractProvider
}

function decodedDisplayValue(value: unknown): unknown {
  if (typeof value === "bigint") return value.toString()
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return value
  }
  if (value == undefined) return undefined
  if (value instanceof Address) {
    return value.toString()
  }
  if (value instanceof Cell) {
    return formatCellHex(value)
  }
  if (value instanceof Dictionary) {
    return Object.fromEntries(
      [...value].map(([key, item]) => [
        String(decodedDisplayValue(key)),
        decodedDisplayValue(item),
      ]),
    )
  }
  if (value instanceof Map) {
    return Object.fromEntries(
      [...value].map(([key, item]) => [
        String(decodedDisplayValue(key)),
        decodedDisplayValue(item),
      ]),
    )
  }
  if (Array.isArray(value)) {
    return value.map(item => decodedDisplayValue(item))
  }
  if (isCellLike(value)) {
    return formatCellHex(value.asCell())
  }
  if (isRecord(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, decodedDisplayValue(item)]),
    )
  }
  return stringifyUnknown(value)
}

function formatCellHex(cell: Cell): string {
  return cell.toBoc().toString("hex")
}

function isCellLike(value: unknown): value is CellLike {
  return isRecord(value) && typeof value.asCell === "function"
}

function isPlainDecodedValue(value: unknown): boolean {
  return (
    value == undefined ||
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  )
}

function formatDecodedTolkValue(value: unknown, symbols: SymTable, tyIdx: number): string {
  return formatDecodedTolkNode(value, symbols, tyIdx, 0)
}

function formatDecodedTolkNode(
  value: unknown,
  symbols: SymTable,
  tyIdx: number | undefined,
  indent: number,
): string {
  const pad = " ".repeat(indent * 4)
  const nextPad = " ".repeat((indent + 1) * 4)

  if (Array.isArray(value)) {
    if (value.length === 0) return "[]"
    const itemTyIdx = tyIdx === undefined ? undefined : getCollectionItemTyIdx(symbols, tyIdx)
    return `[\n${value
      .map(item => `${nextPad}${formatDecodedTolkNode(item, symbols, itemTyIdx, indent + 1)}`)
      .join("\n")}\n${pad}]`
  }

  if (isRecord(value)) {
    const entries = Object.entries(value).filter(([key]) => key !== "$")
    const typeName =
      typeof value.$ === "string"
        ? value.$
        : tyIdx === undefined
          ? undefined
          : sanitizeTolkTypeName(formatType(symbols, tyIdx))
    const fields =
      tyIdx === undefined ? new Map<string, StructFieldInfo>() : getStructFields(symbols, tyIdx)

    if (entries.length === 0) {
      return typeName ? `${typeName} {}` : "{}"
    }

    const body = entries
      .map(([key, item]) => {
        const field = fields.get(key)
        const comment = field?.description
          ? `${formatTolkDocComment(field.description, nextPad.length)}\n`
          : ""
        return `${comment}${nextPad}${formatTolkFieldName(key)}: ${formatDecodedTolkNode(
          item,
          symbols,
          field?.tyIdx,
          indent + 1,
        )}`
      })
      .join("\n")
    return `${typeName ? `${typeName} ` : ""}{\n${body}\n${pad}}`
  }

  return formatDecodedTolkScalar(value)
}

function formatDecodedTolkScalar(value: unknown): string {
  if (value == undefined) return "null"
  if (typeof value === "number" || typeof value === "boolean") return String(value)
  if (typeof value === "string") {
    return /^-?\d+$/.test(value) || /^(true|false|null)$/.test(value)
      ? value
      : JSON.stringify(value)
  }
  return JSON.stringify(stringifyUnknown(value))
}

function stringifyUnknown(value: unknown): string {
  if (typeof value === "string") return value
  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {
    return String(value)
  }
  if (value instanceof Error) return value.message
  const json = JSON.stringify(value)
  return json ?? Object.prototype.toString.call(value)
}

function getStructFields(symbols: SymTable, tyIdx: number): Map<string, StructFieldInfo> {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return new Map<string, StructFieldInfo>()

  switch (ty.kind) {
    case "StructRef": {
      const declaration = tryGetStruct(symbols, ty.struct_name)
      const fields = declaration?.fields ?? []
      return new Map<string, StructFieldInfo>(
        fields.map(field => [
          field.name,
          {
            tyIdx: field.client_ty_idx ?? field.ty_idx,
            description: field.description,
          },
        ]),
      )
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined
        ? new Map<string, StructFieldInfo>()
        : getStructFields(symbols, targetTyIdx)
    }
    case "nullable": {
      return getStructFields(symbols, ty.inner_ty_idx)
    }
    default: {
      return new Map<string, StructFieldInfo>()
    }
  }
}

function getCollectionItemTyIdx(symbols: SymTable, tyIdx: number): number | undefined {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return undefined

  switch (ty.kind) {
    case "arrayOf":
    case "lispListOf":
    case "cellOf": {
      return ty.inner_ty_idx
    }
    case "nullable": {
      return getCollectionItemTyIdx(symbols, ty.inner_ty_idx)
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined ? undefined : getCollectionItemTyIdx(symbols, targetTyIdx)
    }
    default: {
      return undefined
    }
  }
}

function formatTolkDocComment(description: string, indentSpaces: number): string {
  const pad = " ".repeat(indentSpaces)
  return description
    .split(/\r?\n/)
    .map(line => `${pad}/// ${line.trim()}`)
    .join("\n")
}

function formatTolkIdentifier(value: string): string {
  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(value)) {
    return value
  }
  return `\`${value.replaceAll("\\", "\\\\").replaceAll("`", "\\`")}\``
}

function formatTolkFieldName(value: string): string {
  return formatTolkIdentifier(value)
}

function sanitizeTolkTypeName(value: string): string {
  return value.replace(/\?.*$/, "").trim()
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}

function formatGetMethodSignature(method: ABIGetMethod, symbols: SymTable): string {
  const params = method.parameters
    .map(
      parameter =>
        `${formatTolkIdentifier(parameter.name)}: ${formatType(symbols, parameter.ty_idx)}`,
    )
    .join(", ")
  return `get fun ${formatTolkIdentifier(method.name)}(${params}): ${formatType(
    symbols,
    method.return_ty_idx,
  )}`
}

function formatArgsPlaceholder(method: ABIGetMethod, symbols: SymTable): string {
  return JSON.stringify(
    method.parameters.map(parameter => sampleValueForTy(symbols, parameter.ty_idx)),
    undefined,
    2,
  )
}

function getSimpleArgInput(
  symbols: SymTable,
  tyIdx: number,
  nullable = false,
): AbiSimpleArgInput | undefined {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return undefined

  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return {kind: "number", nullable}
    }
    case "bool": {
      return {kind: "bool", nullable}
    }
    case "string": {
      return {kind: "string", nullable}
    }
    case "address":
    case "addressExt": {
      return {kind: "address", nullable}
    }
    case "addressOpt":
    case "addressAny": {
      return {kind: "address", nullable: true}
    }
    case "cell":
    case "builder":
    case "slice":
    case "remaining":
    case "bitsN": {
      return {kind: "cell", nullable}
    }
    case "nullable": {
      return getSimpleArgInput(symbols, ty.inner_ty_idx, true)
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined
        ? undefined
        : getSimpleArgInput(symbols, targetTyIdx, nullable)
    }
    default: {
      return undefined
    }
  }
}

function initialSimpleArgValue(symbols: SymTable, tyIdx: number): string {
  const input = getSimpleArgInput(symbols, tyIdx)
  if (!input) return ""
  switch (input.kind) {
    case "bool": {
      return input.nullable ? "" : "false"
    }
    default: {
      return ""
    }
  }
}

function placeholderForSimpleArg(input: AbiSimpleArgInput): string {
  if (input.nullable) return "empty = null"
  switch (input.kind) {
    case "number": {
      return "0"
    }
    case "string": {
      return "value"
    }
    case "address": {
      return "EQ..."
    }
    case "cell": {
      return "0x... or base64 BoC"
    }
    case "bool": {
      return ""
    }
  }
}

function sampleValueForTy(symbols: SymTable, tyIdx: number, visited = new Set<number>()): unknown {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return undefined
  switch (ty.kind) {
    case "int":
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "coins":
    case "EnumRef": {
      return "0"
    }
    case "bool": {
      return false
    }
    case "string": {
      return ""
    }
    case "address":
    case "addressOpt":
    case "addressExt":
    case "addressAny": {
      return "EQ..."
    }
    case "cell":
    case "slice":
    case "builder":
    case "bitsN":
    case "remaining": {
      return "te6ccgEBAQEAAgAAAA=="
    }
    case "nullable": {
      return undefined
    }
    case "cellOf": {
      return {ref: sampleValueForTy(symbols, ty.inner_ty_idx, visited)}
    }
    case "arrayOf":
    case "lispListOf": {
      return []
    }
    case "tensor":
    case "shapedTuple": {
      return ty.items_ty_idx.map(itemTyIdx => sampleValueForTy(symbols, itemTyIdx, visited))
    }
    case "mapKV": {
      return {}
    }
    case "StructRef": {
      if (visited.has(tyIdx)) return {}
      visited.add(tyIdx)
      return Object.fromEntries(
        symbols
          .structFieldsOf(tyIdx, true)
          .map(field => [field.name, sampleValueForTy(symbols, field.ty_idx, visited)]),
      )
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined ? undefined : sampleValueForTy(symbols, targetTyIdx, visited)
    }
    case "union": {
      return {$: "Variant", value: undefined}
    }
    case "nullLiteral": {
      return undefined
    }
    default: {
      return undefined
    }
  }
}

function formatType(symbols: SymTable, tyIdx: number): string {
  try {
    return renderTy(symbols, tyIdx)
  } catch {
    const ty = tryTyByIdx(symbols, tyIdx)
    return ty ? formatTyFallback(ty, symbols) : "unknown"
  }
}

function formatAbiTyDeclaration(symbols: SymTable, tyIdx: number): string {
  const declaration = getAbiTyDeclaration(symbols, tyIdx)
  return declaration
    ? formatDeclarationTolk(declaration, symbols)
    : formatTypeBlock(symbols, tyIdx, 0)
}

function getAbiTyDeclaration(symbols: SymTable, tyIdx: number): AbiDeclaration | undefined {
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) return undefined
  switch (ty.kind) {
    case "StructRef": {
      return tryGetStruct(symbols, ty.struct_name)
    }
    case "AliasRef": {
      return tryGetAlias(symbols, ty.alias_name)
    }
    case "EnumRef": {
      return tryGetEnum(symbols, ty.enum_name)
    }
    default: {
      return undefined
    }
  }
}

function formatTypeBlock(
  symbols: SymTable,
  tyIdx: number,
  depth: number,
  visited = new Set<number>(),
): string {
  if (visited.has(tyIdx)) {
    return formatType(symbols, tyIdx)
  }
  const ty = tryTyByIdx(symbols, tyIdx)
  if (!ty) {
    return "unknown"
  }
  visited.add(tyIdx)

  switch (ty.kind) {
    case "StructRef": {
      const fields = symbols.structFieldsOf(tyIdx, false)
      if (fields.length === 0) {
        return `${formatTolkIdentifier(ty.struct_name)} {}`
      }
      const baseIndent = "    ".repeat(depth)
      const fieldIndent = "    ".repeat(depth + 1)
      const fieldLines = fields
        .map(field => {
          const renderedType = formatTypeBlock(
            symbols,
            field.ty_idx,
            depth + 1,
            new Set(visited),
          ).trimStart()
          return `${fieldIndent}${formatTolkFieldName(field.name)}: ${renderedType}`
        })
        .join("\n")
      return `${baseIndent}${formatTolkIdentifier(ty.struct_name)} {\n${fieldLines}\n${baseIndent}}`
    }
    case "AliasRef": {
      const targetTyIdx = tryAliasTargetTyIdx(symbols, tyIdx)
      return targetTyIdx === undefined
        ? formatTolkIdentifier(ty.alias_name)
        : `${"    ".repeat(depth)}${formatTolkIdentifier(ty.alias_name)} =\n${formatTypeBlock(
            symbols,
            targetTyIdx,
            depth + 1,
            visited,
          )}`
    }
    case "union": {
      return `${"    ".repeat(depth)}${ty.variants
        .map(variant => {
          const prefix = formatTolkPrefix({
            prefix_num: variant.prefix_num,
            prefix_len: variant.prefix_len,
          })
          return `${formatTypeBlock(symbols, variant.variant_ty_idx, depth + 1, new Set(visited))}${
            prefix ? ` /* ${prefix} */` : ""
          }`
        })
        .join(`\n${"    ".repeat(depth)}| `)}`
    }
    case "nullable": {
      return `${formatTypeBlock(symbols, ty.inner_ty_idx, depth, visited)}?`
    }
    default: {
      return `${"    ".repeat(depth)}${formatType(symbols, tyIdx)}`
    }
  }
}

function formatTyFallback(ty: Ty, symbols: SymTable): string {
  switch (ty.kind) {
    case "intN":
    case "uintN":
    case "varintN":
    case "varuintN":
    case "bitsN": {
      return `${ty.kind}<${ty.n}>`
    }
    case "StructRef": {
      return formatGenericName(ty.struct_name, ty.type_args_ty_idx, symbols)
    }
    case "AliasRef": {
      return formatGenericName(ty.alias_name, ty.type_args_ty_idx, symbols)
    }
    case "EnumRef": {
      return formatTolkIdentifier(ty.enum_name)
    }
    case "nullable": {
      return `${formatType(symbols, ty.inner_ty_idx)}?`
    }
    case "cellOf":
    case "arrayOf":
    case "lispListOf": {
      return `${ty.kind}<${formatType(symbols, ty.inner_ty_idx)}>`
    }
    case "tensor":
    case "shapedTuple": {
      return `${ty.kind}<${ty.items_ty_idx.map(itemTyIdx => formatType(symbols, itemTyIdx)).join(", ")}>`
    }
    case "mapKV": {
      return `map<${formatType(symbols, ty.key_ty_idx)}, ${formatType(symbols, ty.value_ty_idx)}>`
    }
    case "genericT": {
      return ty.name_t
    }
    case "union": {
      return ty.variants.map(variant => formatType(symbols, variant.variant_ty_idx)).join(" | ")
    }
    default: {
      return ty.kind
    }
  }
}

function formatGenericName(
  name: string,
  typeArgsTyIdx: readonly number[] | undefined,
  symbols: SymTable,
): string {
  if (typeArgsTyIdx === undefined || typeArgsTyIdx.length === 0) {
    return formatTolkIdentifier(name)
  }
  return `${formatTolkIdentifier(name)}<${typeArgsTyIdx
    .map(tyIdx => formatType(symbols, tyIdx))
    .join(", ")}>`
}

function formatDeclarationTolk(declaration: AbiDeclaration, symbols: SymTable): string {
  switch (declaration.kind) {
    case "struct": {
      const prefix = declaration.prefix ? ` (${formatTolkPrefix(declaration.prefix)})` : ""
      if (declaration.fields.length === 0) {
        return `struct${prefix} ${formatTolkIdentifier(declaration.name)} {}`
      }
      const fields = declaration.fields
        .map(field => {
          const comment = field.description ? `${formatTolkDocComment(field.description, 4)}\n` : ""
          return `${comment}    ${formatTolkFieldName(field.name)}: ${formatType(
            symbols,
            field.client_ty_idx ?? field.ty_idx,
          )}`
        })
        .join("\n")
      return `struct${prefix} ${formatTolkIdentifier(declaration.name)} {\n${fields}\n}`
    }
    case "alias": {
      return `type ${formatTolkIdentifier(declaration.name)} = ${formatType(
        symbols,
        declaration.target_ty_idx,
      )}`
    }
    case "enum": {
      const members = declaration.members
        .map(member => {
          const description = (member as AbiEnumMemberWithDescription).description
          const comment = description ? `${formatTolkDocComment(description, 4)}\n` : ""
          return `${comment}    ${formatTolkIdentifier(member.name)} = ${member.value}`
        })
        .join("\n")
      return `enum ${formatTolkIdentifier(declaration.name)} {\n${members}\n}`
    }
  }
}

function tryTyByIdx(symbols: SymTable, tyIdx: number): Ty | undefined {
  try {
    return symbols.tyByIdx(tyIdx)
  } catch {
    return undefined
  }
}

function tryGetStruct(
  symbols: SymTable,
  structName: string,
): Extract<AbiDeclaration, {kind: "struct"}> | undefined {
  try {
    return symbols.getStruct(structName)
  } catch {
    return undefined
  }
}

function tryGetAlias(
  symbols: SymTable,
  aliasName: string,
): Extract<AbiDeclaration, {kind: "alias"}> | undefined {
  try {
    return symbols.getAlias(aliasName)
  } catch {
    return undefined
  }
}

function tryGetEnum(
  symbols: SymTable,
  enumName: string,
): Extract<AbiDeclaration, {kind: "enum"}> | undefined {
  try {
    return symbols.getEnum(enumName)
  } catch {
    return undefined
  }
}

function tryAliasTargetTyIdx(symbols: SymTable, tyIdx: number): number | undefined {
  try {
    return symbols.aliasTargetOf(tyIdx).ty_idx
  } catch {
    return undefined
  }
}

function formatTolkPrefix(prefix: {
  readonly prefix_num: number
  readonly prefix_len: number
}): string {
  if (prefix.prefix_len % 4 === 0) {
    return `0x${(prefix.prefix_num >>> 0)
      .toString(16)
      .padStart(Math.max(1, prefix.prefix_len / 4), "0")}`
  }
  return `0b${prefix.prefix_num.toString(2).padStart(prefix.prefix_len, "0")}`
}

function renderDeclarationBody(declaration: AbiDeclaration, symbols: SymTable): JSX.Element {
  return (
    <div className={styles.abiDeclarationBody}>
      <TolkCode value={formatDeclarationTolk(declaration, symbols)} />
      {declaration.description && (
        <p className={styles.abiDeclarationDescription}>{declaration.description}</p>
      )}
    </div>
  )
}

function scrollToAbiSymbol(target: HTMLElement | null): void {
  if (!target) {
    return
  }

  const headerOffset = 116
  const top = target.getBoundingClientRect().top + globalThis.scrollY - headerOffset
  globalThis.scrollTo({top: Math.max(0, top), behavior: "auto"})
}

function declarationKindClass(kind: AbiDeclaration["kind"]): string {
  switch (kind) {
    case "struct": {
      return styles.abiDeclarationKindStruct
    }
    case "alias": {
      return styles.abiDeclarationKindAlias
    }
    case "enum": {
      return styles.abiDeclarationKindEnum
    }
  }
}

function TolkCode({
  value,
  wrap = false,
}: {
  readonly value: string
  readonly wrap?: boolean
}): JSX.Element {
  return (
    <div className={styles.abiTolkCode}>
      <CodeContent value={value} language="tolk" wrap={wrap} />
    </div>
  )
}

function CodeContent({
  value,
  language,
  wrap,
}: {
  readonly value: string
  readonly language?: HighlightLanguage
  readonly wrap: boolean
}): JSX.Element {
  if (language) {
    return <HighlightedCode value={value} language={language} wrap={wrap} />
  }

  return (
    <pre className={`${styles.code} ${wrap ? styles.codeWrap : ""}`}>
      <code>{value}</code>
    </pre>
  )
}

function HighlightedCode({
  value,
  language,
  wrap,
}: {
  readonly value: string
  readonly language: HighlightLanguage
  readonly wrap: boolean
}): JSX.Element {
  const [highlightedHtml, setHighlightedHtml] = useState<string | undefined>()

  useEffect(() => {
    let isActive = true

    const highlight = async () => {
      setHighlightedHtml(undefined)
      try {
        const highlighter = await getAbiHighlighter()
        const isDark = document.documentElement.classList.contains("dark-theme")
        const html = highlighter.codeToHtml(value, {
          lang: language,
          theme: isDark ? "jetbrains-darcula" : "jetbrains-light",
        })

        if (isActive) {
          setHighlightedHtml(html)
        }
      } catch (error) {
        console.error("Failed to highlight ABI code:", error)
        if (isActive) {
          setHighlightedHtml(undefined)
        }
      }
    }

    void highlight()

    const observer = new MutationObserver(mutations => {
      for (const mutation of mutations) {
        if (mutation.type === "attributes" && mutation.attributeName === "class") {
          void highlight()
        }
      }
    })
    observer.observe(document.documentElement, {attributes: true})

    return () => {
      isActive = false
      observer.disconnect()
    }
  }, [language, value])

  if (!highlightedHtml) {
    return (
      <pre className={`${styles.code} ${wrap ? styles.codeWrap : ""}`}>
        <code>{value}</code>
      </pre>
    )
  }

  return (
    <div
      className={`${styles.highlightedCode} ${wrap ? styles.highlightedCodeWrap : ""}`}
      dangerouslySetInnerHTML={{__html: highlightedHtml}}
    />
  )
}
