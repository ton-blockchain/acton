import {Upload} from "lucide-react"
import {useEffect, useId, useRef, useState} from "react"
import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {InlineButton} from "../InlineButton"
import inputStyles from "../Input/Input.module.css"
import styles from "./BocInput.module.css"

/**
 * Text and file input for serialized cells. Callers own BoC decoding, root-cell
 * constraints and submission; file-read failures are reported through onError.
 */
export interface BocInputProps
  extends Omit<ComponentPropsWithRef<"textarea">, "value" | "onChange" | "onError"> {
  readonly value: string
  readonly onValueChange: (value: string) => void
  readonly onError: (error: Error) => void
  /** Lets a form suspend submission until the selected file replaces the current value */
  readonly onReadingChange?: (reading: boolean) => void
  readonly label?: ReactNode
  readonly description?: ReactNode
  readonly invalid?: boolean
  readonly fieldClassName?: string
  readonly maxFileBytes?: number
}

/** Preserves pasted text and converts binary BoC files to base64 without decoding cells */
export function BocInput({
  value,
  onValueChange,
  onError,
  onReadingChange,
  label = "Cell",
  description = "Base64, base64url, hex or a link containing a BoC",
  invalid = false,
  disabled = false,
  readOnly = false,
  maxFileBytes = 12 * 1024 * 1024,
  fieldClassName,
  className,
  id,
  rows = 7,
  placeholder = "Paste a BoC or load a file",
  "aria-describedby": ariaDescribedBy,
  ...props
}: BocInputProps) {
  const generatedId = useId()
  const inputId = id ?? generatedId
  const descriptionId = `${inputId}-description`
  const fileInput = useRef<HTMLInputElement>(null)
  const readRevision = useRef(0)
  const [reading, setReading] = useState(false)

  useEffect(() => {
    onReadingChange?.(reading)

    return () => {
      // Switching actions can unmount this input before a file read completes.
      // Release the parent form while readRevision discards that stale result.
      if (reading) onReadingChange?.(false)
    }
  }, [onReadingChange, reading])

  useEffect(
    () => () => {
      readRevision.current += 1
    },
    [],
  )

  async function loadFile(file: File) {
    readRevision.current += 1
    const revision = readRevision.current
    setReading(true)

    try {
      if (file.size > maxFileBytes) {
        throw new Error(`Choose a file smaller than ${maxFileBytes.toLocaleString()} bytes`)
      }

      const bytes = new Uint8Array(await file.arrayBuffer())
      const magic = bytes.length >= 4 ? new DataView(bytes.buffer).getUint32(0) : 0
      let text: string

      if ([0xb5_ee_9c_72, 0x68_ff_65_f3, 0xac_c3_a7_28].includes(magic)) {
        // Chunk conversion keeps large binary files below JavaScript's argument limit.
        let binary = ""
        for (let offset = 0; offset < bytes.length; offset += 8192) {
          binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192))
        }

        text = btoa(binary)
      } else {
        text = new TextDecoder("utf-8", {fatal: true}).decode(bytes)
      }

      if (revision === readRevision.current) {
        onValueChange(text)
      }
    } catch (cause) {
      if (revision === readRevision.current) {
        onError(cause instanceof Error ? cause : new Error(String(cause)))
      }
    } finally {
      if (revision === readRevision.current) {
        setReading(false)
      }
    }
  }

  return (
    <div className={cx(inputStyles.field, fieldClassName)}>
      <div className={inputStyles.labelRow}>
        <label className={inputStyles.label} htmlFor={inputId}>
          {label}
        </label>
        <InlineButton
          variant="utility"
          leadingIcon={<Upload />}
          disabled={disabled || readOnly || reading}
          onClick={() => fileInput.current?.click()}
        >
          {reading ? "Reading file…" : "Load file"}
        </InlineButton>
      </div>
      <input
        ref={fileInput}
        type="file"
        accept=".boc,.txt,application/octet-stream,text/plain"
        aria-label="Load BoC file"
        hidden
        disabled={disabled || readOnly || reading}
        onChange={event => {
          const file = event.currentTarget.files?.[0]
          event.currentTarget.value = ""
          if (file) void loadFile(file)
        }}
      />
      <textarea
        {...props}
        id={inputId}
        value={value}
        onChange={event => onValueChange(event.target.value)}
        disabled={disabled || reading}
        readOnly={readOnly}
        rows={rows}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        aria-invalid={invalid || props["aria-invalid"]}
        aria-describedby={
          [ariaDescribedBy, description ? descriptionId : undefined].filter(Boolean).join(" ") ||
          undefined
        }
        className={cx(
          inputStyles.input,
          inputStyles.mono,
          styles.textarea,
          invalid && inputStyles.invalid,
          className,
        )}
      />
      {description && (
        <div id={descriptionId} className={inputStyles.description}>
          {description}
        </div>
      )}
    </div>
  )
}
