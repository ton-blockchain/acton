import {useState} from "react"

import {NftChip} from "@acton/ui"

import styles from "./nftChipGallery.module.css"

const PREVIEW_IMAGE = `data:image/svg+xml,${encodeURIComponent(`
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
    <rect width="64" height="64" rx="12" fill="#2f6fda"/>
    <circle cx="32" cy="25" r="14" fill="#fff"/>
    <path d="M24 25h16M32 17v16" stroke="#2f6fda" stroke-width="4" stroke-linecap="round"/>
    <rect x="10" y="45" width="44" height="9" rx="4.5" fill="#dfeaff"/>
  </svg>
`)}`

export function NftChipGallerySamples() {
  const [openedLabel, setOpenedLabel] = useState<string>()

  return (
    <div className={styles.samples}>
      <article className={styles.sample}>
        <span className={styles.label}>Text fallback</span>
        <NftChip label="NFT #88802769231" />
      </article>
      <article className={styles.sample}>
        <span className={styles.label}>Resolved preview</span>
        <NftChip label="+888 0276 9231" imageSrc={PREVIEW_IMAGE} />
      </article>
      <article className={styles.sample}>
        <span className={styles.label}>Clickable</span>
        <NftChip
          label="+888 0258 5371"
          imageSrc={PREVIEW_IMAGE}
          ariaLabel="Open +888 0258 5371"
          onClick={() => setOpenedLabel("+888 0258 5371")}
        />
        <span className={styles.result} aria-live="polite">
          {openedLabel ? `Opened ${openedLabel}` : "Click the NFT to inspect navigation"}
        </span>
      </article>
    </div>
  )
}
