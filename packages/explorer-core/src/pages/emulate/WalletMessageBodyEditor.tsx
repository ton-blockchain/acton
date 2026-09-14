import {useEffect, useMemo, useRef, useState} from "react"
import {beginCell} from "@ton/core"
import {BocInput, InlineButton, Input, Select, useToast} from "@acton/ui"
import {
  AbiValueEditor,
  buildAbiMessageBody,
  createAbiMessageSymbols,
  decodeAbiMessageBuilderDraft,
  formatAbiMessageOptionSummary,
  listAbiMessageBuilderOptions,
  parseAbiCellArg,
  parseAbiJson,
  stringifyAbiJson,
  type ContractABI,
  type TonAddressSuggestion,
} from "@acton/transaction-ui"

import {EMPTY_MESSAGE_BODY} from "./walletMessages"
import styles from "./WalletMessageEditor.module.css"

type BodyMode = "empty" | "comment" | "abi" | "raw"

interface WalletMessageBodyEditorProps {
  readonly address: string
  readonly value: string
  readonly onChange: (value: string) => void
  readonly disabled: boolean
  readonly loadAbi: (address: string) => Promise<ContractABI | undefined>
  readonly addressSuggestions: readonly TonAddressSuggestion[]
}

/** Reuses the regular ABI message serializer for the body nested inside a wallet send action. */
export function WalletMessageBodyEditor({
  address,
  value,
  onChange,
  disabled,
  loadAbi,
  addressSuggestions,
}: WalletMessageBodyEditorProps) {
  const {showToast} = useToast()
  const [initial] = useState(() => describeBody(value))
  const [mode, setMode] = useState<BodyMode>(initial.mode)
  const [comment, setComment] = useState(initial.comment)
  const [abi, setAbi] = useState<ContractABI>()
  const [loading, setLoading] = useState(false)
  const [messageId, setMessageId] = useState("")
  const [args, setArgs] = useState<unknown>({})
  const [revision, setRevision] = useState(0)
  const latest = useRef({value, onChange})
  const preservedBody = useRef(value)
  const options = useMemo(() => (abi ? listAbiMessageBuilderOptions(abi, "internal") : []), [abi])
  const symbols = useMemo(() => (abi ? createAbiMessageSymbols(abi) : undefined), [abi])
  const selected = options.find(option => option.id === messageId)

  useEffect(() => {
    latest.current = {value, onChange}
  }, [value, onChange])

  useEffect(() => {
    if (mode !== "abi") return
    let active = true
    setAbi(undefined)
    setLoading(true)
    // Block simulation while the recipient's ABI is changing instead of sending the old payload.
    if (latest.current.value) preservedBody.current = latest.current.value
    latest.current.onChange("")

    const timeout = globalThis.setTimeout(() => {
      void loadAbi(address)
        .then(loaded => {
          if (!active) return
          if (!loaded) throw new Error("No ABI found for this recipient")
          const messageOptions = listAbiMessageBuilderOptions(loaded, "internal")
          if (messageOptions.length === 0)
            throw new Error("The recipient ABI has no incoming messages")
          let decoded: ReturnType<typeof decodeAbiMessageBuilderDraft>
          try {
            decoded = decodeAbiMessageBuilderDraft(
              loaded,
              "internal",
              parseAbiCellArg(preservedBody.current),
            )
          } catch {
            /* An unfinished or empty draft does not necessarily match a message. */
          }

          const option = decoded?.option ?? messageOptions[0]
          const argsJson = decoded?.argsJson ?? option.sampleJson
          setAbi(loaded)
          setMessageId(option.id)
          setArgs(parseAbiJson(argsJson))
          latest.current.onChange(
            buildAbiMessageBody({abi: loaded, option, argsJson}).toBoc().toString("hex"),
          )
        })
        .catch(error => {
          if (active) reportError(error)
        })
        .finally(() => {
          if (active) setLoading(false)
        })
    }, 350)

    return () => {
      active = false
      globalThis.clearTimeout(timeout)
    }
  }, [address, loadAbi, mode, revision])

  function reportError(error: unknown) {
    showToast({
      title: "Cannot build message body",
      description: error instanceof Error ? error.message : String(error),
      variant: "error",
    })
  }

  function changeMode(next: BodyMode) {
    if (value) preservedBody.current = value
    setMode(next)
    if (next === "empty") onChange(EMPTY_MESSAGE_BODY)
    if (next === "raw") onChange(preservedBody.current)
    if (next === "comment")
      onChange(
        beginCell().storeUint(0, 32).storeStringTail(comment).endCell().toBoc().toString("hex"),
      )
  }

  function changeArgs(next: unknown, nextId = messageId) {
    setArgs(next)
    setMessageId(nextId)
    const option = options.find(entry => entry.id === nextId)
    if (!abi || !option) return

    try {
      onChange(
        buildAbiMessageBody({abi, option, argsJson: stringifyAbiJson(next)})
          .toBoc()
          .toString("hex"),
      )
    } catch {
      onChange("")
    }
  }

  return (
    <div className={styles.fields}>
      <Select
        label="Message body"
        value={mode}
        disabled={disabled}
        onChange={event => changeMode(event.target.value as BodyMode)}
      >
        <option value="empty">Empty</option>
        <option value="comment">Comment</option>
        <option value="abi">ABI message</option>
        <option value="raw">Raw cell</option>
      </Select>

      {mode === "comment" && (
        <Input
          label="Comment"
          value={comment}
          disabled={disabled}
          onChange={event => {
            setComment(event.target.value)
            try {
              onChange(
                beginCell()
                  .storeUint(0, 32)
                  .storeStringTail(event.target.value)
                  .endCell()
                  .toBoc()
                  .toString("hex"),
              )
            } catch {
              onChange("")
            }
          }}
        />
      )}
      {mode === "raw" && (
        <BocInput
          label="Body cell"
          value={value}
          onValueChange={onChange}
          onError={reportError}
          description="Hex or base64 BoC"
          rows={3}
          disabled={disabled}
        />
      )}
      {mode === "abi" && (
        <>
          <Select
            label="Message"
            value={messageId}
            disabled={disabled || loading || !abi}
            labelAction={
              <InlineButton
                variant="utility"
                onClick={() => setRevision(current => current + 1)}
                disabled={disabled || loading}
              >
                Reload ABI
              </InlineButton>
            }
            onChange={event => {
              const option = options.find(entry => entry.id === event.target.value)
              if (option) changeArgs(parseAbiJson(option.sampleJson), option.id)
            }}
          >
            {!abi && (
              <option value="">{loading ? "Loading recipient ABI…" : "ABI unavailable"}</option>
            )}
            {options.map(option => (
              <option value={option.id} key={option.id}>
                {formatAbiMessageOptionSummary(option)}
              </option>
            ))}
          </Select>
          {selected && symbols && (
            <AbiValueEditor
              key={selected.id}
              symbols={symbols}
              tyIdx={selected.valueTyIdx}
              value={args}
              onChange={changeArgs}
              disabled={disabled}
              addressSuggestions={addressSuggestions}
            />
          )}
        </>
      )}
    </div>
  )
}

function describeBody(value: string): {readonly mode: BodyMode; readonly comment: string} {
  try {
    const slice = parseAbiCellArg(value).beginParse()
    if (slice.remainingBits === 0 && slice.remainingRefs === 0) return {mode: "empty", comment: ""}
    if (slice.remainingBits >= 32 && slice.loadUint(32) === 0) {
      return {mode: "comment", comment: slice.loadStringTail()}
    }
  } catch {
    /* Keep arbitrary input editable as a raw cell. */
  }
  return {mode: "raw", comment: ""}
}
