import {useEffect, useMemo, useRef, useState, type ReactNode} from "react"
import {HighlightedCode, InlineButton, RawDataBlock} from "@acton/ui"
import {
  callGetMethodDynamic,
  DynamicCtx,
  type ABIGetMethod,
  type ContractABI,
  type SymTable,
} from "@ton/tolk-abi-to-typescript"
import {Play} from "lucide-react"

import {normalizeAbiDynamicArg, sampleAbiValueForTy} from "../../lib/abiValue"
import {AbiValueEditor} from "../AbiValueEditor/AbiValueEditor"
import type {TonAddressSuggestion} from "../TonAddressInput/TonAddressInput"
import {formatAbiDecodedValue} from "./abiDecodedValue"
import {
  createAbiGetMethodProvider,
  type AbiGetMethodResponse,
  type AbiRunGetMethod,
} from "./abiGetMethodStack"
import {AbiMethodSignature} from "./AbiMethodSignature"
import {AbiSection, AbiSymbolAnchor, abiSymbolAnchorId, TolkCode} from "./abiShared"
import styles from "./AbiViewer.module.css"

export interface AbiGetMethodsProps {
  readonly abi: ContractABI
  readonly runGetMethod: AbiRunGetMethod
  readonly addressSuggestions?: readonly TonAddressSuggestion[]
}

type AbiGetMethodRunState =
  | {readonly status: "idle"}
  | {readonly status: "loading"}
  | {readonly status: "success"; readonly result: AbiGetMethodResponse; readonly decoded: unknown}
  | {readonly status: "error"; readonly error: string; readonly result?: AbiGetMethodResponse}

export function AbiGetMethods({abi, runGetMethod, addressSuggestions = []}: AbiGetMethodsProps) {
  const ctx = useMemo(() => new DynamicCtx(abi), [abi])

  return (
    <div className={styles.methodsView}>
      {abi.get_methods.length > 0 ? (
        <div className={styles.methodList}>
          {abi.get_methods.map(method => (
            <AbiRunnableGetMethod
              key={`${method.name}:${method.tvm_method_id}`}
              method={method}
              ctx={ctx}
              errors={abi.thrown_errors}
              runGetMethod={runGetMethod}
              addressSuggestions={addressSuggestions}
            />
          ))}
        </div>
      ) : (
        <div className={styles.emptyInline}>No get methods declared</div>
      )}
    </div>
  )
}

export function AbiReadonlyGetMethodsSection({
  methods,
  symbols,
  showSymbolAnchors,
}: {
  readonly methods: readonly ABIGetMethod[]
  readonly symbols: SymTable
  readonly showSymbolAnchors: boolean
}) {
  return (
    <AbiSection title="Get methods" count={methods.length}>
      {methods.length > 0 ? (
        <div className={styles.methodList}>
          {methods.map(method => (
            <AbiGetMethodOverview
              key={`${method.name}:${method.tvm_method_id}`}
              method={method}
              symbols={symbols}
              showMethodId
              showSymbolAnchors={showSymbolAnchors}
            />
          ))}
        </div>
      ) : (
        <div className={styles.emptyInline}>No get methods declared</div>
      )}
    </AbiSection>
  )
}

function AbiRunnableGetMethod({
  method,
  ctx,
  errors,
  runGetMethod,
  addressSuggestions,
}: {
  readonly method: ABIGetMethod
  readonly ctx: DynamicCtx
  readonly errors: ContractABI["thrown_errors"]
  readonly runGetMethod: AbiRunGetMethod
  readonly addressSuggestions: readonly TonAddressSuggestion[]
}) {
  const [values, setValues] = useState<readonly unknown[]>(() =>
    method.parameters.map(parameter => sampleAbiValueForTy(ctx.symbols, parameter.ty_idx)),
  )
  const [runState, setRunState] = useState<AbiGetMethodRunState>({status: "idle"})
  const requestIdRef = useRef(0)

  useEffect(() => {
    requestIdRef.current += 1
    setValues(
      method.parameters.map(parameter => sampleAbiValueForTy(ctx.symbols, parameter.ty_idx)),
    )
    setRunState({status: "idle"})
  }, [ctx, method])

  const resolvedAbiError =
    runState.status === "error" && runState.result && runState.result.exit_code !== 0
      ? errors.find(error => error.err_code === runState.result?.exit_code)
      : undefined
  const resolvedAbiErrorName = resolvedAbiError?.name?.trim()

  const runMethod = async () => {
    const requestId = ++requestIdRef.current
    let args: readonly unknown[]
    try {
      args = method.parameters.map((parameter, index) =>
        normalizeAbiDynamicArg(ctx, parameter.ty_idx, values[index]),
      )
    } catch (runError) {
      if (requestId === requestIdRef.current) {
        setRunState({
          status: "error",
          error: runError instanceof Error ? runError.message : String(runError),
        })
      }
      return
    }

    setRunState({status: "loading"})
    let result: AbiGetMethodResponse | undefined
    try {
      const provider = createAbiGetMethodProvider(
        runGetMethod,
        value => {
          result = value
        },
        {symbols: ctx.symbols, returnTyIdx: method.return_ty_idx},
      )
      const decoded: unknown = await callGetMethodDynamic(provider, ctx, method.name, [...args])
      if (!result) throw new Error("Get method response was not captured.")
      if (requestId !== requestIdRef.current) return
      setRunState({status: "success", result, decoded})
    } catch (runError) {
      if (requestId !== requestIdRef.current) return
      setRunState({
        status: "error",
        error: runError instanceof Error ? runError.message : String(runError),
        result,
      })
    }
  }

  return (
    <AbiGetMethodOverview
      method={method}
      symbols={ctx.symbols}
      showMethodId={false}
      showSymbolAnchors={false}
      action={
        <InlineButton
          className={styles.methodAction}
          leadingIcon={<Play size={15} />}
          onClick={() => void runMethod()}
          disabled={runState.status === "loading"}
        >
          {runState.status === "loading" ? "Running" : "Run"}
        </InlineButton>
      }
    >
      {method.parameters.length > 0 && (
        <div className={styles.arguments}>
          {method.parameters.map((parameter, index) => (
            <AbiValueEditor
              key={`${parameter.name}:${index}`}
              symbols={ctx.symbols}
              tyIdx={parameter.ty_idx}
              label={parameter.name}
              value={values[index]}
              onChange={value => {
                setValues(current =>
                  current.map((currentValue, valueIndex) =>
                    valueIndex === index ? value : currentValue,
                  ),
                )
                requestIdRef.current += 1
                setRunState({status: "idle"})
              }}
              addressSuggestions={addressSuggestions}
            />
          ))}
        </div>
      )}
      {runState.status === "loading" && <AbiGetMethodSkeleton />}
      {runState.status === "error" && (
        <>
          <div className={styles.methodError}>
            {resolvedAbiErrorName && runState.result ? (
              <>
                Get method exited with{" "}
                <a
                  className={styles.methodErrorLink}
                  href={`#${abiSymbolAnchorId(
                    "error",
                    resolvedAbiErrorName,
                    String(runState.result.exit_code),
                  )}`}
                >
                  {resolvedAbiErrorName}
                </a>{" "}
                ({runState.result.exit_code}).
              </>
            ) : (
              runState.error
            )}
          </div>
          {runState.result && (
            <div className={styles.result}>
              <AbiGetMethodExecutionDetails result={runState.result} />
            </div>
          )}
        </>
      )}
      {runState.status === "success" && (
        <AbiGetMethodResult
          result={runState.result}
          decoded={runState.decoded}
          method={method}
          symbols={ctx.symbols}
        />
      )}
    </AbiGetMethodOverview>
  )
}

function AbiGetMethodOverview({
  method,
  symbols,
  showMethodId,
  showSymbolAnchors,
  action,
  children,
}: {
  readonly method: ABIGetMethod
  readonly symbols: SymTable
  readonly showMethodId: boolean
  readonly showSymbolAnchors: boolean
  readonly action?: ReactNode
  readonly children?: ReactNode
}) {
  const methodId = abiSymbolAnchorId("get-method", method.name)

  return (
    <article id={showSymbolAnchors ? methodId : undefined} className={styles.method}>
      <div className={styles.signatureLine}>
        <AbiMethodSignature method={method} symbols={symbols} />
        {showMethodId && <sup className={styles.methodId}>method id: {method.tvm_method_id}</sup>}
        <AbiSymbolAnchor show={showSymbolAnchors} id={methodId} label={`Link to ${method.name}`} />
        {action}
      </div>
      {method.description && <p className={styles.methodDescription}>{method.description}</p>}
      {children}
    </article>
  )
}

function AbiGetMethodSkeleton() {
  return (
    <div className={styles.resultSkeleton} aria-label="Running get method">
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
  readonly result: AbiGetMethodResponse
  readonly decoded: unknown
  readonly method: ABIGetMethod
  readonly symbols: SymTable
}) {
  const formatted = formatAbiDecodedValue(decoded, symbols, method.return_ty_idx)

  return (
    <div className={styles.result}>
      <div className={styles.decodedResult}>
        {formatted.kind === "plain" ? (
          <span>{formatted.value}</span>
        ) : (
          <div className={styles.decodedTolk}>
            <TolkCode value={formatted.value} />
          </div>
        )}
      </div>
      <AbiGetMethodExecutionDetails result={result} />
    </div>
  )
}

function AbiGetMethodExecutionDetails({result}: {readonly result: AbiGetMethodResponse}) {
  const stackJson = JSON.stringify(result.stack, undefined, 2)

  return (
    <div className={styles.executionDetails}>
      <div className={styles.resultStats}>
        <span>
          <strong>Exit code:</strong> {result.exit_code}
        </span>
        <span>
          <strong>Gas used:</strong> {result.gas_used}
        </span>
      </div>
      <details className={styles.details}>
        <summary>Raw stack JSON</summary>
        <RawDataBlock
          className={styles.detailsBlock}
          value={stackJson}
          copyLabel="stack JSON"
          customContent={<HighlightedCode value={stackJson} language="json" wrap />}
        />
      </details>
      {result.vm_log && (
        <details className={styles.details}>
          <summary>VM log</summary>
          <pre>{result.vm_log}</pre>
        </details>
      )}
    </div>
  )
}
