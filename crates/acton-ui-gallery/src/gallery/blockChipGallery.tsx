import {BlockChip} from "@acton/ui"

import styles from "./addressChipGallery.module.css"
import type {ComponentGallery} from "./types"

export const blockChipGallery = {
  id: "block-chip",
  title: "BlockChip",
  status: "ready",
  summary:
    "BlockChip renders compact block seqnos with shared explorer link, hover, focus, and coordinated highlight styling.",
  importStatement: 'import {BlockChip} from "@acton/ui"',
  agentSummary:
    "Use BlockChip for linked or read-only block seqnos while keeping route construction in the caller.",
  usage: [
    "Pass href for a navigable block seqno.",
    "Use highlighted to coordinate a block with another selected or hovered view.",
    "Keep workchain and shard context in the caller.",
  ],
  avoid: [
    "Do not add visual variants.",
    "Do not fetch block data inside BlockChip.",
    "Do not pre-style the surrounding link; BlockChip owns its interaction surface.",
  ],
  sections: [
    {
      id: "block-chip-states",
      title: "Display States",
      description: "Read-only, linked, and coordinated highlight states.",
      content: (
        <div className={styles.grid}>
          <article className={styles.sample}>
            <span className={styles.sampleTitle}>Read-only</span>
            <BlockChip workchain={-1} shard="8000000000000000" seqno={80323933} />
          </article>
          <article className={styles.sample}>
            <span className={styles.sampleTitle}>Linked</span>
            <BlockChip
              workchain={-1}
              shard="8000000000000000"
              seqno={80323933}
              href="#block-chip"
            />
          </article>
          <article className={styles.sample}>
            <span className={styles.sampleTitle}>Highlighted</span>
            <BlockChip workchain={-1} shard="8000000000000000" seqno={80323933} highlighted />
          </article>
        </div>
      ),
    },
  ],
} satisfies ComponentGallery
