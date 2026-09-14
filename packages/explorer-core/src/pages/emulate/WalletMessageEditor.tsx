import {useCallback, useEffect, useId, useRef, useState} from "react"
import {Disclosure, InlineButton, Input, useToast} from "@acton/ui"
import {
  AbiValueEditor,
  type AbiFieldEditorProps,
  type AbiValueEditorProps,
  type ContractABI,
} from "@acton/transaction-ui"
import {Clock3, RefreshCw} from "lucide-react"

import {WalletMessageListEditor} from "./WalletMessageListEditor"
import type {WalletExternalSchema} from "./walletMessages"
import styles from "./WalletMessageEditor.module.css"

/** Fields resolved from the same account state and timestamp that the simulator will use. */
export type WalletParameter = "seqno" | "walletId" | "validUntil"

interface WalletMessageEditorProps extends AbiValueEditorProps {
  readonly wallet: WalletExternalSchema
  readonly ignoreChksig: boolean
  readonly fetchParameter: (name: WalletParameter) => Promise<string>
  readonly loadRecipientAbi: (address: string) => Promise<ContractABI | undefined>
  readonly initializeParameters: boolean
}

/** Adds wallet semantics to the catalog ABI without taking ownership of the serialized form draft. */
export function WalletMessageEditor({
  wallet,
  ignoreChksig,
  fetchParameter,
  loadRecipientAbi,
  initializeParameters,
  ...editorProps
}: WalletMessageEditorProps) {
  const signatureDescriptionId = useId()
  const currentValue = useRef(editorProps.value)

  useEffect(() => {
    currentValue.current = editorProps.value
  }, [editorProps.value])

  function changeParameter(name: string, value: unknown) {
    // Automatic field requests may resolve in the same render batch; merge them without losing siblings.
    const next = {...(currentValue.current as Record<string, unknown>), [name]: value}
    currentValue.current = next
    editorProps.onChange(next)
  }

  return (
    <AbiValueEditor
      {...editorProps}
      className={styles.walletEditor}
      renderField={field => {
        if (field.structName === wallet.messagesStruct && field.name === wallet.messagesField) {
          return (
            <WalletMessageListEditor
              version={wallet.version}
              value={field.value}
              onChange={field.onChange}
              disabled={field.disabled}
              rawEditor={field.defaultEditor}
              loadRecipientAbi={loadRecipientAbi}
              addressSuggestions={editorProps.addressSuggestions ?? []}
            />
          )
        }
        if (field.structName !== wallet.signedStruct) return field.defaultEditor

        switch (field.name) {
          case "seqno":
          case "walletId":
          case "subwalletId":
          case "validUntil":
            return (
              <WalletParameterInput
                field={{
                  ...field,
                  onChange: value => changeParameter(field.name, value),
                }}
                name={field.name === "subwalletId" ? "walletId" : field.name}
                fetchParameter={fetchParameter}
                initializeParameters={initializeParameters}
              />
            )
          case "extendedActions":
            return (
              <Disclosure
                label="Extended actions"
                className={styles.disclosure}
                contentClassName={styles.disclosureContent}
                open={(field.value !== null && field.value !== undefined) || undefined}
              >
                {field.defaultEditor}
              </Disclosure>
            )
          case "signature":
            return (
              <div className={styles.fields}>
                <div role="group" aria-describedby={signatureDescriptionId}>
                  {field.defaultEditor}
                </div>
                {ignoreChksig && (
                  <p id={signatureDescriptionId} className={styles.hint}>
                    Any signature is accepted while Ignore CHKSIG is enabled
                  </p>
                )}
              </div>
            )
          default:
            return field.defaultEditor
        }
      }}
    />
  )
}

function WalletParameterInput({
  field,
  name,
  fetchParameter,
  initializeParameters,
}: {
  readonly field: AbiFieldEditorProps
  readonly name: WalletParameter
  readonly fetchParameter: WalletMessageEditorProps["fetchParameter"]
  readonly initializeParameters: boolean
}) {
  const {showToast} = useToast()
  const [loading, setLoading] = useState(false)
  const latest = useRef({field, fetchParameter})
  const requestId = useRef(0)
  const edited = useRef(false)

  useEffect(() => {
    latest.current = {field, fetchParameter}
  }, [field, fetchParameter])

  useEffect(
    () => () => {
      requestId.current += 1
    },
    [],
  )

  const fetchValue = useCallback(async () => {
    const request = ++requestId.current
    const previousValue = latest.current.field.value
    const fetch = latest.current.fetchParameter
    setLoading(true)

    try {
      const value = await fetch(name)
      // A slow response must not overwrite another account, changed overrides, or a manual edit.
      if (
        request === requestId.current &&
        latest.current.fetchParameter === fetch &&
        latest.current.field.value === previousValue
      ) {
        latest.current.field.onChange(value)
      }
    } catch (error) {
      if (request === requestId.current && latest.current.fetchParameter === fetch) {
        showToast({
          title: `Failed to fetch ${name}`,
          description: error instanceof Error ? error.message : String(error),
          variant: "error",
        })
      }
    } finally {
      if (request === requestId.current) setLoading(false)
    }
  }, [name, showToast])

  useEffect(() => {
    if (
      initializeParameters &&
      !edited.current &&
      String(latest.current.field.value ?? "0") === "0"
    ) {
      void fetchValue()
    }
  }, [fetchValue, fetchParameter, initializeParameters])

  return (
    <Input
      label={
        name === "walletId"
          ? "Wallet ID"
          : name === "seqno"
            ? "Sequence number (seqno)"
            : "Expires at (validUntil)"
      }
      aria-label={name}
      value={String(field.value ?? "")}
      inputMode="numeric"
      disabled={field.disabled}
      onChange={event => {
        edited.current = true
        requestId.current += 1
        setLoading(false)
        field.onChange(event.target.value)
      }}
      labelAction={
        <InlineButton
          variant="utility"
          title={
            name === "validUntil"
              ? "Set expiration to 5 minutes after the simulation time"
              : "Read from the selected block and state overrides"
          }
          leadingIcon={name === "validUntil" ? <Clock3 /> : <RefreshCw />}
          disabled={field.disabled || loading}
          onClick={() => void fetchValue()}
        >
          {loading ? "Fetching…" : name === "validUntil" ? "Set now + 5 minutes" : "Fetch current"}
        </InlineButton>
      }
      description={
        name === "validUntil"
          ? "The wallet rejects messages after this UNIX timestamp"
          : name === "walletId"
            ? "Identifies the wallet within its network"
            : undefined
      }
    />
  )
}
