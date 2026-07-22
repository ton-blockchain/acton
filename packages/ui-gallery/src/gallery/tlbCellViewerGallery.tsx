import {TlbCellViewer} from "@acton/transaction-ui"
import type {ReactNode} from "react"

import styles from "./tlbCellViewerGallery.module.css"
import {
  ADDRESS_CELL_BOC,
  ADDRESS_CELL_SCHEMA,
  OPAQUE_CELL_BOC,
  OPAQUE_CELL_SCHEMA,
  OPTIONAL_NONE_BOC,
  OPTIONAL_NONE_SCHEMA,
  SCALAR_CELL_BOC,
  SCALAR_CELL_SCHEMA,
  SIMPLE_CELL_BOC,
  SIMPLE_CELL_SCHEMA,
} from "./tlbCellViewerGalleryFixtures"
import type {ComponentGallery} from "./types"

function sample(label: string, viewer: ReactNode) {
  return (
    <article className={styles.sample}>
      <span className={styles.sampleLabel}>{label}</span>
      {viewer}
    </article>
  )
}

export const tlbCellViewerGallery = {
  id: "tlb-cell-viewer",
  title: "TlbCellViewer",
  status: "ready",
  summary:
    "TlbCellViewer parses a base64 TON cell against a TL-B schema and presents its typed value tree, cell metadata, schema, and failures.",
  importStatement: 'import {TlbCellViewer} from "@acton/transaction-ui"',
  agentSummary:
    "Use TlbCellViewer when both the exact TL-B schema and BoC are available. The component owns TL-B decoding and presentation; callers should not pre-parse runtime objects.",
  usage: [
    "Pass the exact schema string associated with the signed or inspected cell.",
    "Use defaultSchemaExpanded in diagnostic contexts where the schema should be reviewed immediately.",
    "Keep the original BoC unchanged so parse failures and raw cell metadata remain trustworthy.",
  ],
  avoid: [
    "Do not use it for cells without a known TL-B schema.",
    "Do not inspect runtime objects in the caller or duplicate the recursive value renderer.",
    "Do not silently replace an invalid schema with a guessed type.",
  ],
  sections: [
    {
      id: "tlb-cell-viewer-values",
      title: "Scalar and Optional Values",
      description:
        "Fixed and variable integers, signed values, booleans, flags, Maybe, and uint256 formatting.",
      content: (
        <div className={styles.viewerGrid}>
          {sample(
            "uint64 · VarUInteger",
            <TlbCellViewer boc={SIMPLE_CELL_BOC} schema={SIMPLE_CELL_SCHEMA} />,
          )}
          {sample(
            "int · Bool · flags · Maybe · uint256",
            <TlbCellViewer
              boc={SCALAR_CELL_BOC}
              schema={SCALAR_CELL_SCHEMA}
              defaultSchemaExpanded
            />,
          )}
          {sample(
            "Maybe None",
            <TlbCellViewer boc={OPTIONAL_NONE_BOC} schema={OPTIONAL_NONE_SCHEMA} />,
          )}
        </div>
      ),
    },
    {
      id: "tlb-cell-viewer-references",
      title: "Addresses and Cell References",
      description:
        "Typed nested references remain structured, while an opaque ^Cell keeps its own bits, refs, hash, and raw BoC.",
      content: (
        <div className={styles.viewerGrid}>
          {sample(
            "MsgAddress · typed ^NestedAudit",
            <TlbCellViewer boc={ADDRESS_CELL_BOC} schema={ADDRESS_CELL_SCHEMA} />,
          )}
          {sample(
            "opaque ^Cell · nested ref",
            <TlbCellViewer boc={OPAQUE_CELL_BOC} schema={OPAQUE_CELL_SCHEMA} />,
          )}
        </div>
      ),
    },
    {
      id: "tlb-cell-viewer-errors",
      title: "Failure States",
      description:
        "Invalid schemas and malformed BoCs stay visible instead of producing partial data.",
      content: (
        <div className={styles.viewerGrid}>
          {sample(
            "schema mismatch",
            <TlbCellViewer
              boc={SIMPLE_CELL_BOC}
              schema="wrong_payload#ffffffff value:uint32 = WrongPayload;"
            />,
          )}
          {sample(
            "invalid base64 BoC",
            <TlbCellViewer boc="not-a-ton-cell" schema={SIMPLE_CELL_SCHEMA} />,
          )}
        </div>
      ),
    },
  ],
} satisfies ComponentGallery
