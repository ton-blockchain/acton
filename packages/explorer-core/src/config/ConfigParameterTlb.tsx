import {HighlightedCode, RawDataBlock} from "@acton/ui"
import {ExternalLink} from "lucide-react"

import {getConfigParameterTlb} from "./tlb"
import styles from "./ConfigParameterTlb.module.css"

/** Loaded only when opening TL-B, keeping the schema catalog out of the initial config view */
export default function ConfigParameterTlb({id}: {readonly id: number}) {
  const source = getConfigParameterTlb(id)
  if (!source) return null

  return (
    <>
      <RawDataBlock
        className={styles.schema}
        title={
          <a
            href={source.sourceUrl}
            target="_blank"
            rel="noreferrer"
            className={styles.sourceLink}
            aria-label="View block.tlb in the TON repository"
          >
            block.tlb <ExternalLink size={14} aria-hidden="true" />
          </a>
        }
        value={source.declaration}
        variant="embedded"
        copyLabel={`parameter ${id} TL-B`}
        customContent={
          <HighlightedCode value={source.declaration} language="tlb" maxHeight="32rem" wrap />
        }
      />
      {source.dependencies && (
        <RawDataBlock
          title="Additional types"
          className={styles.additionalTypes}
          collapsible
          defaultExpanded={false}
          value={source.dependencies}
          variant="embedded"
          copyLabel={`parameter ${id} additional types`}
          customContent={
            <HighlightedCode value={source.dependencies} language="tlb" maxHeight="32rem" wrap />
          }
        />
      )}
    </>
  )
}
