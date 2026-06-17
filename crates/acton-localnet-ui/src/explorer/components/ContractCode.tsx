import {Buffer} from "node:buffer"

import {
  decodeStorageDataCell,
  jetbrainsDarculaTheme,
  jetbrainsLightTheme,
  ParsedValueView,
  type ContractData,
} from "@acton/shared-ui"
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
import {Check, CheckCircle2, Copy, ExternalLink, FileCode2, Folder, Menu, Play} from "lucide-react"
import type React from "react"
import type {CSSProperties} from "react"
import {useEffect, useMemo, useState} from "react"
import {createHighlighterCore} from "shiki/core"
import {createOnigurumaEngine} from "shiki/engine/oniguruma"
import type {LanguageRegistration} from "shiki/types"
import {Cell as Cell2, runtime, text} from "ton-assembly"

import type {TonClient} from "../api/client"
import type {
  SourceBundle,
  SourceFile,
  V3RunGetMethodResponse,
  V3RunGetMethodStackEntry,
  VerificationSourceResponse,
} from "../api/types"
import {useAddressFormat} from "../hooks/useNetworkInfo"
import type {AddressFormatOptions} from "./utils"

import funcGrammarRaw from "../../../../../docs/grammars/grammar-func.json"
import tasmGrammarRaw from "../../../../../docs/grammars/grammar-tasm.json"
import tolkGrammarRaw from "../../../../../docs/grammars/grammar-tolk.json"

import styles from "./ContractCode.module.css"

interface ContractCodeProps {
  readonly codeBoc: string
  readonly ownerAddress: string
  readonly client: TonClient
  readonly dataBoc?: string
  readonly compilerAbi?: ContractABI
  readonly compilerAbiLoading?: boolean
  readonly compilerAbiError?: string
  readonly verifiedSource?: VerificationSourceResponse
  readonly verifiedSourceLoading?: boolean
  readonly onContractClick?: (address: string) => void
}

type ContractCodeTab = "storage" | "source" | "abi"
type StorageTab = "parsed" | "base64" | "hex" | "hex-hash" | "base64-hash"
type SourceTab = "verified" | "decompiled" | "base64" | "hex" | "hex-hash" | "base64-hash"
type AbiTab = "view" | "raw"
type HighlightLanguage = "tasm" | "json" | "tolk" | "func"
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

const grammarWithName = (grammar: unknown, name: string): LanguageRegistration =>
  ({
    ...(grammar as Record<string, unknown>),
    name,
  }) as LanguageRegistration

const tasmGrammar = grammarWithName(tasmGrammarRaw, "tasm")
const tolkGrammar = grammarWithName(tolkGrammarRaw, "tolk")
const funcGrammar = grammarWithName(funcGrammarRaw, "func")

let contractCodeHighlighterPromise: ReturnType<typeof createHighlighterCore> | undefined
const VERIFIER_BASE_URL = "https://verifier.acton.monster"

const getContractCodeHighlighter = () => {
  contractCodeHighlighterPromise ??= createHighlighterCore({
    themes: [jetbrainsLightTheme, jetbrainsDarculaTheme],
    langs: [tasmGrammar, tolkGrammar, funcGrammar, import("shiki/langs/json.mjs")],
    engine: createOnigurumaEngine(() => import("shiki/wasm")),
  })

  return contractCodeHighlighterPromise
}

function readContractHashTab(): ContractCodeTab {
  if (typeof globalThis.window === "undefined") {
    return "storage"
  }

  const hash = globalThis.window.location.hash.replace("#", "")
  if (hash === "contract-source") {
    return "source"
  }
  if (hash === "contract-abi") {
    return "abi"
  }
  return "storage"
}

function writeContractHashTab(tab: ContractCodeTab): void {
  if (typeof globalThis.window === "undefined") {
    return
  }

  const nextUrl = `${globalThis.window.location.pathname}${globalThis.window.location.search}#contract-${tab}`
  globalThis.window.history.replaceState(undefined, "", nextUrl)
}

export const ContractCode: React.FC<ContractCodeProps> = ({
  codeBoc,
  ownerAddress,
  client,
  dataBoc,
  compilerAbi,
  compilerAbiLoading = false,
  compilerAbiError,
  verifiedSource,
  verifiedSourceLoading = false,
  onContractClick,
}) => {
  const [activeTab, setActiveTab] = useState<ContractCodeTab>(() => readContractHashTab())
  const [activeStorageTab, setActiveStorageTab] = useState<StorageTab>("parsed")
  const [activeSourceTab, setActiveSourceTab] = useState<SourceTab>("verified")
  const [activeAbiTab, setActiveAbiTab] = useState<AbiTab>("view")
  const addressFormat = useAddressFormat()

  const codeData = useMemo(() => {
    if (!codeBoc) return
    try {
      const buf = Buffer.from(codeBoc, "base64")
      const cell = Cell2.fromBoc(buf)[0]
      const codeCell = Cell.fromBase64(codeBoc)
      const decompiled = text.print(runtime.decompileCell(cell))

      return {
        base64: codeBoc,
        codeHashBase64: codeCell.hash().toString("base64"),
        codeHashHex: codeCell.hash().toString("hex"),
        hex: buf.toString("hex").toUpperCase(),
        decompiled: decompiled,
      }
    } catch (error) {
      console.error("Failed to process contract code:", error)
      return {
        base64: codeBoc,
        codeHashBase64: "Error processing code hash",
        codeHashHex: "Error processing code hash",
        hex: "Error processing HEX",
        decompiled: "Error: Failed to decompile code.",
      }
    }
  }, [codeBoc])

  const parsedStorage = useMemo(
    () => decodeStorageDataCell(dataBoc, compilerAbi),
    [dataBoc, compilerAbi],
  )
  const storageData = useMemo(() => {
    if (!dataBoc) return
    try {
      const buf = Buffer.from(dataBoc, "base64")
      const dataCell = Cell.fromBase64(dataBoc)

      return {
        base64: dataBoc,
        dataHashBase64: dataCell.hash().toString("base64"),
        dataHashHex: dataCell.hash().toString("hex"),
        hex: buf.toString("hex").toUpperCase(),
      }
    } catch (error) {
      console.error("Failed to process contract data:", error)
      return {
        base64: dataBoc,
        dataHashBase64: "Error processing data hash",
        dataHashHex: "Error processing data hash",
        hex: "Error processing data HEX",
      }
    }
  }, [dataBoc])
  const contracts = useMemo(() => new Map<string, ContractData>(), [])
  const abiJson = useMemo(() => {
    if (!compilerAbi) return
    return JSON.stringify(compilerAbi, undefined, 2)
  }, [compilerAbi])
  const storageUnavailableMessage = compilerAbi
    ? dataBoc
      ? "Storage data could not be decoded with this ABI."
      : "No storage data available for this account."
    : "No compiler ABI registered for storage decoding."
  const hasVerifiedSource = Boolean(verifiedSource?.verified && verifiedSource.bundles.length > 0)

  const handleContractTabChange = (tab: ContractCodeTab) => {
    setActiveTab(tab)
    writeContractHashTab(tab)
  }

  if (!codeBoc || !codeData) {
    return (
      <div className={styles.container}>
        <div className={styles.empty}>No code available for this account.</div>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <div className={styles.tabs}>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "storage" ? styles.tabActive : ""}`}
          onClick={() => handleContractTabChange("storage")}
        >
          Storage
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "source" ? styles.tabActive : ""}`}
          onClick={() => handleContractTabChange("source")}
        >
          Source
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "abi" ? styles.tabActive : ""}`}
          onClick={() => handleContractTabChange("abi")}
        >
          ABI
        </button>
      </div>

      <div className={styles.content}>
        {activeTab === "storage" ? (
          <StoragePanel
            activeTab={activeStorageTab}
            onTabChange={setActiveStorageTab}
            storageData={storageData}
            parsedStorage={parsedStorage}
            contracts={contracts}
            addressFormat={addressFormat}
            onContractClick={onContractClick}
            unavailableMessage={storageUnavailableMessage}
          />
        ) : activeTab === "abi" ? (
          compilerAbiError ? (
            <div className={styles.empty}>Failed to load compiler ABI: {compilerAbiError}</div>
          ) : compilerAbiLoading ? (
            <div className={styles.empty}>Loading compiler ABI...</div>
          ) : compilerAbi && abiJson ? (
            <AbiPanel
              activeTab={activeAbiTab}
              onTabChange={setActiveAbiTab}
              abi={compilerAbi}
              abiJson={abiJson}
              ownerAddress={ownerAddress}
              client={client}
            />
          ) : (
            <div className={styles.empty}>No compiler ABI registered for this contract.</div>
          )
        ) : (
          <SourcePanel
            activeTab={activeSourceTab}
            onTabChange={setActiveSourceTab}
            codeData={codeData}
            verifiedSource={hasVerifiedSource ? verifiedSource : undefined}
          />
        )}
        {verifiedSourceLoading && !hasVerifiedSource && activeTab === "source" && (
          <div className={styles.verifiedLoading}>Checking verified source...</div>
        )}
      </div>
    </div>
  )
}

function StoragePanel({
  activeTab,
  onTabChange,
  storageData,
  parsedStorage,
  contracts,
  addressFormat,
  onContractClick,
  unavailableMessage,
}: {
  readonly activeTab: StorageTab
  readonly onTabChange: (tab: StorageTab) => void
  readonly storageData?: {
    readonly base64: string
    readonly dataHashBase64: string
    readonly dataHashHex: string
    readonly hex: string
  }
  readonly parsedStorage?: ReturnType<typeof decodeStorageDataCell>
  readonly contracts: Map<string, ContractData>
  readonly addressFormat: AddressFormatOptions
  readonly onContractClick?: (address: string) => void
  readonly unavailableMessage: string
}): React.JSX.Element {
  const storageTabs: readonly {tab: StorageTab; label: string}[] = [
    {tab: "parsed", label: "parsed"},
    {tab: "base64", label: "base64"},
    {tab: "hex", label: "hex"},
    {tab: "hex-hash", label: "hex hash"},
    {tab: "base64-hash", label: "base64 hash"},
  ]
  const activeStorage =
    activeTab === "base64"
      ? {
          title: "Data BoC Base64",
          value: storageData?.base64,
          wrap: true,
        }
      : activeTab === "hex"
        ? {
            title: "Data BoC HEX",
            value: storageData?.hex,
            wrap: true,
          }
        : activeTab === "hex-hash"
          ? {
              title: "Data hash HEX",
              value: storageData?.dataHashHex,
              wrap: true,
            }
          : activeTab === "base64-hash"
            ? {
                title: "Data hash Base64",
                value: storageData?.dataHashBase64,
                wrap: true,
              }
            : undefined

  return (
    <section className={styles.sourceShell}>
      <div className={styles.editorTabBar}>
        {storageTabs.map(item => (
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
      {activeTab === "parsed" ? (
        parsedStorage ? (
          <section className={styles.dataPanel}>
            <div className={styles.storageBlock}>
              <ParsedValueView
                value={parsedStorage.value}
                contracts={contracts}
                addressFormat={addressFormat}
                onContractClick={onContractClick}
                fallbackTypeName={parsedStorage.name}
              />
            </div>
          </section>
        ) : (
          <div className={styles.empty}>{unavailableMessage}</div>
        )
      ) : activeStorage?.value ? (
        <ContractTextPanel
          title={activeStorage.title}
          value={activeStorage.value}
          wrap={activeStorage.wrap}
        />
      ) : (
        <div className={styles.empty}>No storage data available for this account.</div>
      )}
    </section>
  )
}

function SourcePanel({
  activeTab,
  onTabChange,
  codeData,
  verifiedSource,
}: {
  readonly activeTab: SourceTab
  readonly onTabChange: (tab: SourceTab) => void
  readonly codeData: {
    readonly base64: string
    readonly codeHashBase64: string
    readonly codeHashHex: string
    readonly hex: string
    readonly decompiled: string
  }
  readonly verifiedSource?: VerificationSourceResponse
}): React.JSX.Element {
  const activeSourceTab = activeTab === "verified" && !verifiedSource ? "decompiled" : activeTab
  const sourceTabs: readonly {
    tab: SourceTab
    label: string
    verified?: boolean
  }[] = [
    ...(verifiedSource ? [{tab: "verified" as const, label: "Verified code", verified: true}] : []),
    {tab: "decompiled", label: "disasm"},
    {tab: "base64", label: "base64"},
    {tab: "hex", label: "hex"},
    {tab: "hex-hash", label: "hex hash"},
    {tab: "base64-hash", label: "base64 hash"},
  ]
  const activeSource =
    activeSourceTab === "verified"
      ? undefined
      : activeSourceTab === "decompiled"
        ? {
            title: "Disassembly",
            value: codeData.decompiled,
            language: "tasm" as const,
            wrap: false,
          }
        : activeSourceTab === "base64"
          ? {
              title: "Code BoC Base64",
              value: codeData.base64,
              wrap: true,
            }
          : activeSourceTab === "hex"
            ? {
                title: "Code BoC HEX",
                value: codeData.hex,
                wrap: true,
              }
            : activeSourceTab === "hex-hash"
              ? {
                  title: "Code hash HEX",
                  value: codeData.codeHashHex,
                  wrap: true,
                }
              : {
                  title: "Code hash Base64",
                  value: codeData.codeHashBase64,
                  wrap: true,
                }

  return (
    <section className={styles.sourceShell}>
      <div className={styles.editorTabBar}>
        {sourceTabs.map(item => (
          <button
            key={item.tab}
            type="button"
            className={`${styles.editorTab} ${item.verified ? styles.editorTabVerified : ""} ${
              activeSourceTab === item.tab ? styles.editorTabActive : ""
            }`}
            onClick={() => onTabChange(item.tab)}
          >
            {item.verified && <CheckCircle2 size={15} aria-hidden="true" />}
            {item.label}
          </button>
        ))}
      </div>
      {activeSourceTab === "verified" && verifiedSource ? (
        <VerifiedSourcePanel source={verifiedSource} />
      ) : activeSource ? (
        <ContractTextPanel
          title={activeSource.title}
          value={activeSource.value}
          language={activeSource.language}
          wrap={activeSource.wrap}
        />
      ) : undefined}
    </section>
  )
}

function AbiPanel({
  activeTab,
  onTabChange,
  abi,
  abiJson,
  ownerAddress,
  client,
}: {
  readonly activeTab: AbiTab
  readonly onTabChange: (tab: AbiTab) => void
  readonly abi: ContractABI
  readonly abiJson: string
  readonly ownerAddress: string
  readonly client: TonClient
}): React.JSX.Element {
  const abiTabs: readonly {tab: AbiTab; label: string}[] = [
    {tab: "view", label: "Rendered"},
    {tab: "raw", label: "Raw JSON"},
  ]

  return (
    <section className={styles.sourceShell}>
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
        <AbiViewPanel abi={abi} ownerAddress={ownerAddress} client={client} />
      ) : (
        <ContractTextPanel title="ABI" value={abiJson} language="json" />
      )}
    </section>
  )
}

function AbiViewPanel({
  abi,
  ownerAddress,
  client,
}: {
  readonly abi: ContractABI
  readonly ownerAddress: string
  readonly client: TonClient
}): React.JSX.Element {
  const ctx = useMemo(() => new DynamicCtx(abi), [abi])
  const symbols = ctx.symbols

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
          ownerAddress={ownerAddress}
          client={client}
        />
        <AbiMessagesSection abi={abi} symbols={symbols} />
        <AbiStorageSection storage={abi.storage} symbols={symbols} />
        <AbiDeclarationsSection declarations={abi.declarations} symbols={symbols} />
        <AbiThrownErrorsSection errors={abi.thrown_errors} />
      </div>
    </section>
  )
}

function AbiGetMethodsSection({
  methods,
  ctx,
  ownerAddress,
  client,
}: {
  readonly methods: readonly ABIGetMethod[]
  readonly ctx: DynamicCtx
  readonly ownerAddress: string
  readonly client: TonClient
}): React.JSX.Element {
  return (
    <AbiSection title="Get methods" count={methods.length}>
      {methods.length > 0 ? (
        <div className={styles.abiMethodList}>
          {methods.map(method => (
            <AbiGetMethodItem
              key={`${method.name}:${method.tvm_method_id}`}
              method={method}
              ctx={ctx}
              ownerAddress={ownerAddress}
              client={client}
            />
          ))}
        </div>
      ) : (
        <div className={styles.abiEmptyInline}>No get methods declared.</div>
      )}
    </AbiSection>
  )
}

function AbiGetMethodItem({
  method,
  ctx,
  ownerAddress,
  client,
}: {
  readonly method: ABIGetMethod
  readonly ctx: DynamicCtx
  readonly ownerAddress: string
  readonly client: TonClient
}): React.JSX.Element {
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
  const argsInputId = `args-${method.name}-${method.tvm_method_id}`
  const symbols = ctx.symbols
  const simpleArgInputs = method.parameters.map(parameter =>
    getSimpleArgInput(symbols, parameter.ty_idx),
  )
  const canRenderSimpleArgs = hasParameters && simpleArgInputs.every(Boolean)

  const runMethod = async () => {
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
    <article className={styles.abiMethod}>
      <div className={styles.abiMethodTopline}>
        <div className={styles.abiSignatureBlock}>
          <div className={styles.abiSignatureLine}>
            <TolkCode value={formatGetMethodSignature(method, symbols)} />
            <sup className={styles.abiMethodId}>method id: {method.tvm_method_id}</sup>
          </div>
        </div>
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
      </div>

      {method.description && <p className={styles.abiMethodDescription}>{method.description}</p>}

      {canRenderSimpleArgs ? (
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
      ) : hasParameters ? (
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

function AbiGetMethodSkeleton(): React.JSX.Element {
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
}): React.JSX.Element {
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
}): React.JSX.Element {
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
}: {
  readonly abi: ContractABI
  readonly symbols: SymTable
}): React.JSX.Element {
  const groups: readonly {
    readonly title: string
    readonly messages: readonly AbiMessage[]
    readonly empty: string
  }[] = [
    {
      title: "Incoming/internal",
      messages: abi.incoming_messages,
      empty: "No incoming internal messages declared.",
    },
    {
      title: "Incoming external",
      messages: abi.incoming_external,
      empty: "No incoming external messages declared.",
    },
    {
      title: "Outgoing",
      messages: abi.outgoing_messages,
      empty: "No outgoing messages declared.",
    },
    {
      title: "Emitted events",
      messages: abi.emitted_events,
      empty: "No emitted events declared.",
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
              group.messages.map((message, index) => (
                <AbiMessageRow
                  key={`${group.title}:${message.body_ty_idx}:${index}`}
                  message={message}
                  symbols={symbols}
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
  message,
  symbols,
}: {
  readonly message: AbiMessage
  readonly symbols: SymTable
}): React.JSX.Element {
  const declaration = getAbiTyDeclaration(symbols, message.body_ty_idx)

  return (
    <div className={styles.abiMessageRow}>
      <TolkCode value={formatAbiTyDeclaration(symbols, message.body_ty_idx)} />
      {declaration?.description && (
        <p className={styles.abiDeclarationDescription}>{declaration.description}</p>
      )}
    </div>
  )
}

function AbiStorageSection({
  storage,
  symbols,
}: {
  readonly storage: ContractABI["storage"]
  readonly symbols: SymTable
}): React.JSX.Element {
  const rows = [
    {label: "storage", tyIdx: storage.storage_ty_idx},
    {
      label: "storageAtDeployment",
      tyIdx: storage.storage_at_deployment_ty_idx,
    },
  ].filter((row): row is {label: string; tyIdx: number} => row.tyIdx !== undefined)

  return (
    <AbiSection title="Storage" count={rows.length}>
      {rows.length > 0 ? (
        <div className={styles.abiRows}>
          {rows.map(row => {
            const declaration = getAbiTyDeclaration(symbols, row.tyIdx)

            return (
              <div key={row.label} className={styles.abiRow}>
                <TolkCode value={formatAbiTyDeclaration(symbols, row.tyIdx)} />
                {declaration?.description && (
                  <p className={styles.abiDeclarationDescription}>{declaration.description}</p>
                )}
              </div>
            )
          })}
        </div>
      ) : (
        <div className={styles.abiEmptyInline}>No storage type indexes declared.</div>
      )}
    </AbiSection>
  )
}

function AbiDeclarationsSection({
  declarations,
  symbols,
}: {
  readonly declarations: readonly AbiDeclaration[]
  readonly symbols: SymTable
}): React.JSX.Element {
  return (
    <AbiSection title="Declarations" count={declarations.length}>
      {declarations.length > 0 ? (
        <div className={styles.abiDeclarationList}>
          {declarations.map(declaration => (
            <details
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
              </summary>
              {renderDeclarationBody(declaration, symbols)}
            </details>
          ))}
        </div>
      ) : (
        <div className={styles.abiEmptyInline}>No declarations emitted.</div>
      )}
    </AbiSection>
  )
}

function AbiThrownErrorsSection({
  errors,
}: {
  readonly errors: readonly ContractABI["thrown_errors"][number][]
}): React.JSX.Element {
  return (
    <AbiSection title="Thrown errors" count={errors.length}>
      {errors.length > 0 ? (
        <div className={styles.abiRows}>
          {errors.map(error => (
            <div
              key={`${error.err_code}:${error.name ?? error.kind}`}
              className={styles.abiErrorRow}
            >
              <span className={styles.abiErrorCode}>{error.err_code}</span>
              <span className={styles.abiErrorName}>{error.name ?? String(error.err_code)}</span>
              {error.description && <span className={styles.abiMuted}>{error.description}</span>}
            </div>
          ))}
        </div>
      ) : (
        <div className={styles.abiEmptyInline}>No thrown errors declared.</div>
      )}
    </AbiSection>
  )
}

function AbiSection({
  title,
  count,
  children,
}: {
  readonly title: string
  readonly count: number
  readonly children: React.ReactNode
}): React.JSX.Element {
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

function VerifiedSourcePanel({
  source,
}: {
  readonly source: VerificationSourceResponse
}): React.JSX.Element {
  const bundles = useMemo(
    () => source.bundles.filter(bundle => bundle.files.length > 0),
    [source.bundles],
  )
  const [selectedBundleHash, setSelectedBundleHash] = useState(bundles[0]?.source_bundle_hash ?? "")
  const activeBundle =
    bundles.find(bundle => bundle.source_bundle_hash === selectedBundleHash) ?? bundles[0]

  useEffect(() => {
    setSelectedBundleHash(bundles[0]?.source_bundle_hash ?? "")
  }, [bundles])

  if (!activeBundle) {
    return <div className={styles.empty}>No verified source files stored for this contract.</div>
  }

  return (
    <section className={styles.verifiedShell}>
      {bundles.length > 1 && (
        <div className={styles.verifiedHeader}>
          <div className={styles.bundleTabs} role="tablist" aria-label="Verified source bundles">
            {bundles.map(bundle => (
              <button
                key={bundle.source_bundle_hash}
                type="button"
                className={`${styles.bundleTab} ${
                  bundle.source_bundle_hash === activeBundle.source_bundle_hash
                    ? styles.bundleTabActive
                    : ""
                }`}
                onClick={() => setSelectedBundleHash(bundle.source_bundle_hash)}
              >
                {shortenMiddle(bundle.source_bundle_hash, 8, 6)}
              </button>
            ))}
          </div>
        </div>
      )}
      <VerifiedCodeViewer
        bundle={activeBundle}
        verificationUrl={`${VERIFIER_BASE_URL}/${encodeURIComponent(source.code_hash)}`}
      />
    </section>
  )
}

function VerifiedCodeViewer({
  bundle,
  verificationUrl,
}: {
  readonly bundle: SourceBundle
  readonly verificationUrl: string
}): React.JSX.Element {
  const entrypointPath = useMemo(
    () => findEntrypointFile(bundle.files, bundle.entrypoint)?.path,
    [bundle.entrypoint, bundle.files],
  )
  const defaultActivePath = entrypointPath ?? bundle.files[0]?.path ?? ""
  const [activePath, setActivePath] = useState(defaultActivePath)
  const [isFileTreeOpen, setFileTreeOpen] = useState(false)

  useEffect(() => {
    setActivePath(defaultActivePath)
    setFileTreeOpen(false)
  }, [bundle.source_bundle_hash, defaultActivePath])

  const activeFile = useMemo(
    () =>
      findFileByPath(bundle.files, activePath) ??
      findFileByPath(bundle.files, entrypointPath) ??
      bundle.files[0],
    [activePath, bundle.files, entrypointPath],
  )
  const tree = useMemo(() => buildFileTree(bundle.files), [bundle.files])
  const code = activeFile ? fileContent(activeFile) : ""
  const language = activeFile ? languageForPath(activeFile.path) : undefined

  if (!activeFile) {
    return <div className={styles.empty}>No verified source files stored for this bundle.</div>
  }

  const selectFile = (path: string) => {
    setActivePath(path)
    setFileTreeOpen(false)
  }

  return (
    <section className={styles.verifiedWorkspace} aria-label="Verified source code">
      <aside className={`${styles.fileTree} ${styles.fileTreeDesktop}`} aria-label="Source files">
        <div className={styles.fileTreeList}>
          <FileTreeRows
            nodes={tree}
            activePath={activeFile.path}
            entrypoint={entrypointPath}
            onSelect={selectFile}
          />
        </div>
      </aside>
      <div className={styles.codePane}>
        <div className={styles.codePaneHeader}>
          <button
            type="button"
            className={`${styles.mobileFileTreeToggle} ${
              isFileTreeOpen ? styles.mobileFileTreeToggleOpen : ""
            }`}
            aria-label="Toggle source files"
            aria-expanded={isFileTreeOpen}
            onClick={() => setFileTreeOpen(current => !current)}
          >
            <Menu size={16} aria-hidden="true" />
          </button>
          <span className={styles.codePanePath} title={activeFile.path}>
            {activeFile.path}
          </span>
          <a
            className={styles.verificationLink}
            href={verificationUrl}
            target="_blank"
            rel="noreferrer"
          >
            <ExternalLink size={13} aria-hidden="true" />
            View verification
          </a>
          <CopyTextButton
            className={styles.codePaneCopyButton}
            title={activeFile.path}
            value={code}
          />
        </div>
        <aside
          className={`${styles.fileTree} ${styles.fileTreeMobile} ${
            isFileTreeOpen ? styles.fileTreeOpen : ""
          }`}
          aria-label="Source files"
        >
          <div className={styles.fileTreeList}>
            <FileTreeRows
              nodes={tree}
              activePath={activeFile.path}
              entrypoint={entrypointPath}
              onSelect={selectFile}
            />
          </div>
        </aside>
        <div className={styles.codeFrame}>
          <div className={styles.lineNumbers} aria-hidden="true">
            {Array.from({length: lineCount(code)}, (_, index) => (
              <span key={index + 1}>{index + 1}</span>
            ))}
          </div>
          <div className={styles.verifiedCode}>
            <CodeContent value={code} language={language} wrap={false} />
          </div>
        </div>
      </div>
    </section>
  )
}

interface FileTreeNode {
  readonly kind: "folder" | "file"
  readonly name: string
  readonly path: string
  readonly children: readonly FileTreeNode[]
  readonly file?: SourceFile
}

interface FileTreeDraftNode {
  readonly kind: "folder" | "file"
  readonly name: string
  readonly path: string
  readonly children: Map<string, FileTreeDraftNode>
  readonly file?: SourceFile
}

function FileTreeRows({
  nodes,
  activePath,
  entrypoint,
  depth = 0,
  onSelect,
}: {
  readonly nodes: readonly FileTreeNode[]
  readonly activePath: string
  readonly entrypoint?: string
  readonly depth?: number
  readonly onSelect: (path: string) => void
}): React.JSX.Element {
  return (
    <>
      {nodes.map(node => {
        const depthStyle = {"--depth": String(depth)} as CSSProperties
        if (node.kind === "folder") {
          return (
            <div key={node.path}>
              <div className={`${styles.fileTreeRow} ${styles.fileTreeFolder}`} style={depthStyle}>
                <Folder size={14} aria-hidden="true" />
                <span>{node.name}</span>
              </div>
              <FileTreeRows
                nodes={node.children}
                activePath={activePath}
                entrypoint={entrypoint}
                depth={depth + 1}
                onSelect={onSelect}
              />
            </div>
          )
        }

        return (
          <button
            key={node.path}
            type="button"
            className={`${styles.fileTreeRow} ${styles.fileTreeFile} ${
              node.path === activePath ? styles.fileTreeRowActive : ""
            }`}
            style={depthStyle}
            title={node.path}
            aria-current={node.path === activePath ? "true" : undefined}
            onClick={() => onSelect(node.path)}
          >
            <FileCode2 size={14} aria-hidden="true" />
            <span>{node.name}</span>
            {node.path === entrypoint && <span className={styles.fileTreeEntrypoint}>main</span>}
          </button>
        )
      })}
    </>
  )
}

function fileContent(file: SourceFile): string {
  const content = file.content_text ?? Buffer.from(file.content_base64, "base64").toString("utf8")
  return content.endsWith("\n") ? content.slice(0, -1) : content
}

function languageForPath(path: string): HighlightLanguage | undefined {
  const normalizedPath = path.toLowerCase()
  if (normalizedPath.endsWith(".tolk")) {
    return "tolk"
  }
  if (normalizedPath.endsWith(".fc") || normalizedPath.endsWith(".func")) {
    return "func"
  }
  if (
    normalizedPath.endsWith(".json") ||
    normalizedPath.endsWith(".abi") ||
    normalizedPath.endsWith(".pkg")
  ) {
    return "json"
  }
  return undefined
}

function lineCount(code: string): number {
  return code.length === 0 ? 1 : code.split("\n").length
}

function normalizeFilePath(path: string): string {
  return path.replaceAll("\\", "/").replace(/^\.?\//, "")
}

function findFileByPath(
  files: readonly SourceFile[],
  path: string | undefined,
): SourceFile | undefined {
  if (!path) {
    return undefined
  }

  const normalizedPath = normalizeFilePath(path)
  return (
    files.find(file => file.path === path) ??
    files.find(file => normalizeFilePath(file.path) === normalizedPath)
  )
}

function findEntrypointFile(
  files: readonly SourceFile[],
  entrypoint: string | undefined,
): SourceFile | undefined {
  const exactMatch = findFileByPath(files, entrypoint)
  if (exactMatch || !entrypoint) {
    return exactMatch
  }

  const normalizedEntrypoint = normalizeFilePath(entrypoint)
  const suffix = `/${normalizedEntrypoint}`
  const suffixMatches = files.filter(file => normalizeFilePath(file.path).endsWith(suffix))
  return suffixMatches.length === 1 ? suffixMatches[0] : undefined
}

function buildFileTree(files: readonly SourceFile[]): readonly FileTreeNode[] {
  const root = new Map<string, FileTreeDraftNode>()

  for (const file of files) {
    const parts = normalizeFilePath(file.path).split("/").filter(Boolean)
    let currentLevel = root
    let currentPath = ""

    for (const [index, part] of parts.entries()) {
      currentPath = currentPath ? `${currentPath}/${part}` : part
      const isFile = index === parts.length - 1
      let node = currentLevel.get(part)
      if (!node) {
        node = {
          kind: isFile ? "file" : "folder",
          name: part,
          path: currentPath,
          children: new Map(),
        }
        currentLevel.set(part, node)
      }

      if (isFile) {
        node = {
          ...node,
          kind: "file",
          file,
        }
        currentLevel.set(part, node)
      }

      currentLevel = node.children
    }
  }

  return sortTree([...root.values()].map(node => freezeTree(node)))
}

function freezeTree(node: FileTreeDraftNode): FileTreeNode {
  return {
    kind: node.kind,
    name: node.name,
    path: node.path,
    children: sortTree([...node.children.values()].map(child => freezeTree(child))),
    file: node.file,
  }
}

function sortTree(nodes: readonly FileTreeNode[]): FileTreeNode[] {
  return [...nodes].sort((left, right) => {
    if (left.kind !== right.kind) {
      return left.kind === "folder" ? -1 : 1
    }
    return left.name.localeCompare(right.name)
  })
}

function shortenMiddle(value: string, prefix = 8, suffix = 6): string {
  if (value.length <= prefix + suffix + 1) {
    return value
  }
  return `${value.slice(0, prefix)}...${value.slice(-suffix)}`
}

type AbiDeclaration = Readonly<ContractABI["declarations"][number]>
type AbiMessage = Readonly<
  | ContractABI["incoming_messages"][number]
  | ContractABI["incoming_external"][number]
  | ContractABI["outgoing_messages"][number]
>

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
    case "lispListOf": {
      return ty.inner_ty_idx
    }
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
        .map(member => `    ${formatTolkIdentifier(member.name)} = ${member.value}`)
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

function renderDeclarationBody(declaration: AbiDeclaration, symbols: SymTable): React.JSX.Element {
  return (
    <div className={styles.abiDeclarationBody}>
      <TolkCode value={formatDeclarationTolk(declaration, symbols)} />
      {declaration.description && (
        <p className={styles.abiDeclarationDescription}>{declaration.description}</p>
      )}
    </div>
  )
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
}): React.JSX.Element {
  return (
    <div className={styles.abiTolkCode}>
      <CodeContent value={value} language="tolk" wrap={wrap} />
    </div>
  )
}

function ContractTextPanel({
  title,
  value,
  language,
  wrap = false,
}: {
  readonly title: string
  readonly value: string
  readonly language?: HighlightLanguage
  readonly wrap?: boolean
}): React.JSX.Element {
  return (
    <section className={styles.dataPanel}>
      <CopyTextButton className={styles.copyButton} title={title} value={value} />
      <CodeContent value={value} language={language} wrap={wrap} />
    </section>
  )
}

function CopyTextButton({
  className,
  title,
  value,
}: {
  readonly className: string
  readonly title: string
  readonly value: string
}): React.JSX.Element {
  const [isCopied, setIsCopied] = useState(false)

  useEffect(() => {
    if (!isCopied) {
      return
    }

    const timer = setTimeout(() => setIsCopied(false), 1600)
    return () => clearTimeout(timer)
  }, [isCopied])

  return (
    <button
      type="button"
      className={className}
      onClick={() => {
        void navigator.clipboard.writeText(value)
        setIsCopied(true)
      }}
      aria-label={isCopied ? `${title} copied` : `Copy ${title}`}
      title={isCopied ? "Copied" : `Copy ${title}`}
    >
      {isCopied ? <Check size={14} /> : <Copy size={14} />}
    </button>
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
}): React.JSX.Element {
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
}): React.JSX.Element {
  const [highlightedHtml, setHighlightedHtml] = useState<string | undefined>()

  useEffect(() => {
    let isActive = true

    const highlight = async () => {
      setHighlightedHtml(undefined)
      try {
        const highlighter = await getContractCodeHighlighter()
        const isDark = document.documentElement.classList.contains("dark-theme")
        const html = highlighter.codeToHtml(value, {
          lang: language,
          theme: isDark ? "jetbrains-darcula" : "jetbrains-light",
        })

        if (isActive) {
          setHighlightedHtml(html)
        }
      } catch (error) {
        console.error("Failed to highlight contract code:", error)
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
