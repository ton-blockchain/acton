import {createElement} from "react"

import {NftChipGallerySamples} from "./nftChipGallerySamples"
import type {ComponentGallery} from "./types"

export const nftChipGallery = {
  id: "nft-chip",
  title: "NftChip",
  status: "ready",
  summary:
    "NftChip presents a compact NFT identity with an optional preview and native navigation interaction.",
  importStatement: 'import {NftChip} from "@acton/ui"',
  agentSummary:
    "Use NftChip after domain code has resolved an NFT label and optional image URL. Keep metadata fetching and routing in the caller.",
  usage: [
    "Pass a concise resolved label and optional image URL.",
    "Use onImageError when the application has its own fallback chain.",
    "Pass onClick directly when the NFT should navigate.",
  ],
  avoid: [
    "Do not fetch NFT metadata inside NftChip.",
    "Do not wrap NftChip in another interactive element.",
    "Do not add hover underlines or outlines beyond the keyboard focus ring.",
  ],
  sections: [
    {
      id: "nft-chip-states",
      title: "Display and Interaction States",
      description:
        "Text fallback, resolved preview, and clickable navigation share the same compact value treatment.",
      content: createElement(NftChipGallerySamples),
    },
  ],
} satisfies ComponentGallery
