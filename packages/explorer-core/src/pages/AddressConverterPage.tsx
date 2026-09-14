import {useEffect, useId, useMemo, useState} from "react"
import {useSearchParams} from "react-router"
import {Address} from "@ton/core"
import {ArrowRightLeft} from "lucide-react"
import {EmptyState, Input} from "@acton/ui"

import {AddressFormats} from "../components/AddressFormats"

import styles from "./AddressConverterPage.module.css"

/** Keeps conversion input shareable while showing both networks independently of the explorer network. */
export function AddressConverterPage({
  onOpenAddress,
}: {
  readonly onOpenAddress: (address: string, testOnly: boolean | undefined) => void
}) {
  const errorId = useId()
  const [searchParams, setSearchParams] = useSearchParams()
  const urlInput = searchParams.get("address") ?? ""
  const [input, setInput] = useState(urlInput)

  // Typing must update synchronously; router transitions otherwise restore the old value and move the caret.
  // URL changes still restore shared links and browser history into the field.
  useEffect(() => setInput(urlInput), [urlInput])
  const result = useMemo(() => readAddress(input), [input])
  const workchainName = new Map([
    [-1, "Masterchain"],
    [0, "Basechain"],
  ]).get(result?.address?.workChain ?? Number.NaN)

  const changeInput = (value: string) => {
    setInput(value)
    setSearchParams(
      current => {
        const next = new URLSearchParams(current)
        if (value) next.set("address", value)
        else next.delete("address")
        return next
      },
      {replace: true},
    )
  }

  return (
    <section className={styles.container}>
      <div className={styles.inputPanel}>
        <Input
          aria-label="Address or explorer link"
          size="lg"
          className={styles.addressInput}
          placeholder="EQ…, UQ…, 0:… or https://…/address/…"
          value={input}
          onChange={event => changeInput(event.target.value)}
          invalid={result?.error !== undefined}
          aria-describedby={result?.error ? errorId : undefined}
          mono
        />
        {/* Keep validation outside Input so changing error state never remounts the focused control. */}
        {result?.error && (
          <div id={errorId} className={styles.inputError}>
            {result.error}
          </div>
        )}
      </div>

      <div className={styles.workspace}>
        {result?.address ? (
          <>
            <dl className={styles.details}>
              <div>
                <dt>Workchain</dt>
                <dd>
                  {result.address.workChain}
                  {workchainName && (
                    <span className={styles.workchainName}> ({workchainName})</span>
                  )}
                </dd>
              </div>
              <div>
                <dt>Input format</dt>
                <dd>{result.inputFormat}</dd>
              </div>
              <div>
                <dt>Input testnet-only flag</dt>
                <dd>{result.testnetFlag}</dd>
              </div>
            </dl>
            <div className={styles.resultHeader}>
              <h1>Address formats</h1>
            </div>
            <AddressFormats address={result.address.toRawString()} onAddressClick={onOpenAddress} />
          </>
        ) : (
          <EmptyState
            className={styles.empty}
            icon={<ArrowRightLeft size={22} />}
            title={result?.error ? "Check the address" : "Convert a TON address"}
            description={
              result?.error
                ? "Correct the input to see its address formats"
                : "Get the address format your wallet or API needs, and check its workchain and testnet-only flag"
            }
          />
        )}
      </div>
    </section>
  )
}

/** Explorer links are inspected as text only; validation and checksum checks belong to @ton/core. */
function readAddress(input: string):
  | {
      readonly address?: Address
      readonly inputFormat?: string
      readonly testnetFlag?: string
      readonly error?: string
    }
  | undefined {
  let value = input.trim()
  if (!value) return undefined

  try {
    if (/^https?:\/\//i.test(value)) {
      const candidate = addressFromExplorerLink(value)
      if (!candidate) return {error: "The link must contain an account address in /address/…"}
      value = candidate
    }

    if (Address.isFriendly(value)) {
      const friendly = Address.parseFriendly(value)
      // Friendly addresses encode a signed byte; @ton/core only sign-extends workchain -1.
      const address = new Address(
        Int8Array.of(friendly.address.workChain)[0],
        friendly.address.hash,
      )
      return {
        address,
        inputFormat: friendly.isBounceable ? "Bounceable" : "Non-bounceable",
        testnetFlag: friendly.isTestOnly ? "Set" : "Not set",
      }
    }

    // parseRaw accepts partial numeric strings, so validate the complete representation first.
    if (!/^-?\d+:[a-f\d]{64}$/i.test(value)) {
      return {
        error: "Enter a 48-character friendly address or workchain: followed by 64 hex characters",
      }
    }
    const address = Address.parseRaw(value)
    if (address.workChain < -128 || address.workChain > 127) {
      return {error: "Friendly addresses require a workchain between -128 and 127"}
    }
    return {address, inputFormat: "Raw", testnetFlag: "Not encoded"}
  } catch (error) {
    return {
      error: /checksum/i.test(String(error))
        ? "Invalid address checksum — check for a typo or an incomplete address"
        : "Invalid TON address — check the address format and encoding",
    }
  }
}

/** Preserves a trailing slash when it is part of a standard-base64 address, rather than a URL separator. */
function addressFromExplorerLink(value: string): string | undefined {
  const match = new URL(value).pathname.match(/\/address\/(.+)$/)
  if (!match) return undefined

  const address = decodeURIComponent(match[1])
  return Address.isFriendly(address) ? address : address.replace(/\/$/, "")
}
