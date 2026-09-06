import {
  Button,
  ContentTabs,
  DialogActions,
  Disclosure,
  Input,
  RawDataBlock,
  useToast,
} from "@acton/ui"
import {useEffect, useState, type ReactNode} from "react"
import type {Cell} from "@ton/core"

import {getConfigParameterMetadata, type NetworkConfigParameter} from "../../api/config"
import {ConfigFields} from "./ConfigFields"
import {ParameterReview} from "./ParameterReview"
import {ParameterErrorDescription} from "./ParameterErrorDescription"
import {
  decodeParameter,
  defaultDraft,
  editableParameterIds,
  encodeParameter,
  parameterShape,
  parseParameterBoc,
  type Draft,
} from "./codec"
import styles from "./ConfigEditor.module.css"

export interface ParameterUpdate {
  readonly index: number
  readonly boc: string
  readonly expectedHash: string | null
}

interface ParameterEditorProps {
  readonly parameter?: NetworkConfigParameter
  readonly busy: boolean
  readonly onApply: (update: ParameterUpdate) => void
  readonly onCancel: () => void
  /** The host places actions in its pinned footer while the editor owns their state */
  readonly renderActions: (actions: ReactNode) => ReactNode
}

/** An application supplies mutation ownership; this editor owns lossless cell input and review. */
export function ParameterEditor(props: ParameterEditorProps) {
  const [indexText, setIndexText] = useState(props.parameter ? String(props.parameter.id) : "")
  const index = /^-?\d+$/.test(indexText) ? Number(indexText) : Number.NaN
  const validIndex = Number.isInteger(index) && index >= -2_147_483_648 && index <= 2_147_483_647

  const parameterSelector = !props.parameter && (
    <>
      <Input
        label="Parameter number"
        value={indexText}
        inputMode="numeric"
        list="config-parameter-numbers"
        description="Choose a known parameter or enter any signed 32-bit index"
        onChange={event => setIndexText(event.currentTarget.value)}
      />
      <datalist id="config-parameter-numbers">
        {editableParameterIds.map(id => (
          <option key={id} value={id}>
            {getConfigParameterMetadata(id).title}
          </option>
        ))}
      </datalist>
    </>
  )

  return (
    <div className={styles.group}>
      {parameterSelector}
      {validIndex ? (
        <ParameterValueEditor key={index} {...props} index={index} />
      ) : (
        <>
          <p className={styles.hint}>Enter a parameter number to choose its value</p>
          {props.renderActions(
            <DialogActions>
              <Button variant="outline" disabled={props.busy} onClick={props.onCancel}>
                Cancel
              </Button>
            </DialogActions>,
          )}
        </>
      )}
    </div>
  )
}

function initialValue(parameter: NetworkConfigParameter | undefined, index: number) {
  const shape = parameterShape(index)
  try {
    const draft = parameter
      ? decodeParameter(index, parseParameterBoc(parameter.rawHex))
      : shape
        ? defaultDraft(shape)
        : undefined
    return {draft, error: undefined}
  } catch (error) {
    return {draft: undefined, error: error instanceof Error ? error.message : String(error)}
  }
}

function ParameterValueEditor({
  parameter,
  index,
  busy,
  onApply,
  onCancel,
  renderActions,
}: ParameterEditorProps & {readonly index: number}) {
  const {showToast} = useToast()
  const [initial] = useState(() => initialValue(parameter, index))
  const [draft, setDraft] = useState<Draft | undefined>(initial.draft)
  const [mode, setMode] = useState<"fields" | "raw">(draft === undefined ? "raw" : "fields")
  const [raw, setRaw] = useState(parameter?.rawHex ?? "")
  const [review, setReview] = useState<Cell>()
  const shape = parameterShape(index)
  const metadata = getConfigParameterMetadata(index)

  useEffect(() => {
    if (initial.error) {
      showToast({
        id: `config-parameter-${index}-decode`,
        title: "Could not decode parameter",
        description: <ParameterErrorDescription error={initial.error} />,
        variant: "error",
      })
    }
  }, [index, initial.error, showToast])

  const build = () =>
    mode === "fields" && draft !== undefined
      ? encodeParameter(index, draft)
      : parseParameterBoc(raw)

  const reviewChange = () => {
    try {
      const cell = build()
      if (parameter && cell.equals(parseParameterBoc(parameter.rawHex))) {
        showToast({
          title: "No changes to apply",
          description: "The new value matches the current value on the network",
          variant: "info",
        })
        return
      }

      // Known parameters must satisfy the same TL-B constraints in raw mode too.
      if (shape) decodeParameter(index, cell)
      setReview(cell)
    } catch (cause) {
      showToast({
        title: "Invalid parameter value",
        description: <ParameterErrorDescription error={cause} />,
        variant: "error",
      })
    }
  }

  const changeMode = (next: "fields" | "raw") => {
    if (next === mode) return

    try {
      if (next === "raw" && draft !== undefined)
        setRaw(encodeParameter(index, draft).toBoc().toString("hex"))
      if (next === "fields") setDraft(decodeParameter(index, parseParameterBoc(raw)))
      setMode(next)
    } catch (cause) {
      showToast({
        title: "Cannot change input format",
        description: <ParameterErrorDescription error={cause} />,
        variant: "error",
      })
    }
  }

  const fields =
    mode === "fields" && shape && draft !== undefined ? (
      <ConfigFields
        shape={shape}
        value={draft}
        context={{parameter: index, field: ""}}
        onChange={setDraft}
      />
    ) : (
      <div className={styles.rawField}>
        <label className={styles.rawLabel} htmlFor="parameter-boc">
          Parameter BoC (hex or base64)
        </label>
        <textarea
          id="parameter-boc"
          className={styles.raw}
          value={raw}
          onChange={event => setRaw(event.currentTarget.value)}
          spellCheck={false}
        />
      </div>
    )

  const content = (
    <div className={styles.group}>
      {!parameter && (
        <p className={styles.hint}>
          {metadata.title} — {metadata.description.replace(/\.$/, "")}
        </p>
      )}
      {review ? (
        <>
          <p className={styles.hint}>
            Review the new value before applying it to the running network
          </p>
          <ParameterReview index={index} before={parameter} after={review} />

          <Disclosure label="Technical details">
            <div className={styles.group}>
              <RawDataBlock
                title="New parameter hash"
                value={review.hash().toString("hex")}
                maxHeight="6rem"
              />
              <RawDataBlock
                title="New parameter BoC"
                value={review.toBoc().toString("hex")}
                maxHeight="12rem"
              />
            </div>
          </Disclosure>
        </>
      ) : shape ? (
        <ContentTabs
          ariaLabel="Parameter input format"
          variant="standalone"
          value={mode}
          onValueChange={changeMode}
          tabs={[
            {label: "Fields", value: "fields"},
            {label: "Raw cell", value: "raw"},
          ]}
        >
          {fields}
        </ContentTabs>
      ) : (
        fields
      )}
    </div>
  )

  const actions = (
    <DialogActions>
      <Button
        variant="outline"
        disabled={busy}
        onClick={review ? () => setReview(undefined) : onCancel}
      >
        {review ? "Back to editing" : "Cancel"}
      </Button>
      <Button
        variant="primary"
        loading={busy}
        onClick={
          review
            ? () =>
                onApply({
                  index,
                  boc: review.toBoc().toString("base64"),
                  expectedHash: parameter
                    ? parseParameterBoc(parameter.rawHex).hash().toString("hex")
                    : null,
                })
            : reviewChange
        }
      >
        {review ? "Apply to network" : "Review change"}
      </Button>
    </DialogActions>
  )

  return (
    <>
      {content}
      {renderActions(actions)}
    </>
  )
}
