import {AccountOgImage, type AccountOgPreview} from "./AccountOgImage"

export type PageOgKey =
  | "home"
  | "config"
  | "blocks"
  | "block"
  | "abi"
  | "sources"
  | "faucet"
  | "verified"
  | "verified-statistics"
  | "verified-contract"
  | "cell"
  | "emulate"
  | "favorites"
  | "suspended"
  | "transaction"

export type PageOgPreview = {
  readonly key: PageOgKey
  readonly title: string
  readonly badge: string
  readonly description: string
  readonly metadataTitle: string
  readonly metadataDescription: string
}

const PAGE_OG_PREVIEWS: Record<PageOgKey, PageOgPreview> = {
  home: {
    key: "home",
    title: "actonscan",
    badge: "TON explorer",
    description: "Accounts, transactions, blocks, tokens and collectibles",
    metadataTitle: "actonscan · TON explorer",
    metadataDescription:
      "Open-source TON explorer for accounts, transactions, blocks, tokens, and collectibles.",
  },
  config: {
    key: "config",
    title: "Config",
    badge: "Network configuration",
    description:
      "Readable TON network parameters for validators, fees, bridges and system contracts",
    metadataTitle: "TON network configuration · actonscan",
    metadataDescription:
      "Browse readable TON network configuration parameters, validators, fees, bridges, and system contracts on actonscan.",
  },
  blocks: {
    key: "blocks",
    title: "Blocks",
    badge: "Network",
    description: "Follow masterchain and shardchain activity as TON moves, block by block",
    metadataTitle: "TON blocks · actonscan",
    metadataDescription: "Browse recent TON masterchain and shardchain blocks on actonscan.",
  },
  block: {
    key: "block",
    title: "Block",
    badge: "Block details",
    description: "See every transaction and message behind the block",
    metadataTitle: "TON block · actonscan",
    metadataDescription: "Inspect a TON block, its transactions, and messages on actonscan.",
  },
  abi: {
    key: "abi",
    title: "ABI",
    badge: "Contract interfaces",
    description: "Understand contracts through their messages, methods and errors",
    metadataTitle: "TON ABI catalog · actonscan",
    metadataDescription:
      "Browse known TON contract interfaces, messages, methods, and error codes on actonscan.",
  },
  sources: {
    key: "sources",
    title: "Sources",
    badge: "Verified code",
    description: "Read the verified code behind on-chain contracts",
    metadataTitle: "Verified TON sources · actonscan",
    metadataDescription: "Browse verified TON smart-contract source code on actonscan.",
  },
  faucet: {
    key: "faucet",
    title: "Testnet Faucet",
    badge: "Proof of work",
    description: "Fund a TON Testnet address without leaving the explorer",
    metadataTitle: "TON Testnet Faucet · actonscan",
    metadataDescription: "Request testnet GRAM through Acton's proof-of-work protected faucet",
  },
  verified: {
    key: "verified",
    title: "Verified contracts",
    badge: "Source registry",
    description: "Browse reproducible source bundles matched to deployed TON contract code",
    metadataTitle: "Verified TON contracts · actonscan",
    metadataDescription:
      "Browse verified TON smart-contract source bundles, compilers, and code hashes on actonscan.",
  },
  "verified-statistics": {
    key: "verified-statistics",
    title: "Verification stats",
    badge: "Source registry",
    description: "Explore verified contracts by language and compiler version",
    metadataTitle: "TON verification statistics · actonscan",
    metadataDescription:
      "Explore verified TON smart contracts by source language and compiler version on actonscan.",
  },
  "verified-contract": {
    key: "verified-contract",
    title: "Verified contract",
    badge: "Source bundle",
    description: "Review the exact sources and compiler settings matched to on-chain code",
    metadataTitle: "Verified TON contract · actonscan",
    metadataDescription:
      "Review verified source code, compiler metadata, and source bundles for a TON contract on actonscan.",
  },
  cell: {
    key: "cell",
    title: "Cell Inspector",
    badge: "BOC tools",
    description: "Turn raw BOCs into a graph you can actually explore",
    metadataTitle: "TON Cell Inspector · actonscan",
    metadataDescription: "Decode TON BOCs and inspect their cell graphs on actonscan.",
  },
  emulate: {
    key: "emulate",
    title: "Emulate",
    badge: "Transaction tools",
    description: "Test TON messages against chain state before sending",
    metadataTitle: "Emulate TON transactions · actonscan",
    metadataDescription: "Build and emulate TON messages against chain state on actonscan.",
  },
  favorites: {
    key: "favorites",
    title: "Favorites",
    badge: "Watchlist",
    description: "Keep favorite accounts and transactions one click away",
    metadataTitle: "TON favorites · actonscan",
    metadataDescription: "Open your saved TON accounts and transactions on actonscan.",
  },
  suspended: {
    key: "suspended",
    title: "Suspended addresses",
    badge: "Validators' voting",
    description: "Review TON addresses suspended through validators' voting and their balances",
    metadataTitle: "Suspended TON addresses · actonscan",
    metadataDescription:
      "Browse TON addresses suspended through validators' voting and check when restrictions expire on actonscan.",
  },
  transaction: {
    key: "transaction",
    title: "Transaction",
    badge: "Execution details",
    description: "Follow every message, phase, fee and state change",
    metadataTitle: "TON transaction · actonscan",
    metadataDescription:
      "Inspect a TON transaction, messages, fees, and state changes on actonscan.",
  },
}

export function pageOgPreviewForKey(key: string): PageOgPreview {
  return PAGE_OG_PREVIEWS[key as PageOgKey] ?? PAGE_OG_PREVIEWS.home
}

export function pageOgPreviewForPath(pathname: string): PageOgPreview | undefined {
  const normalizedPath = normalizePath(pathname)
  if (normalizedPath === "/") return PAGE_OG_PREVIEWS.home
  if (/^\/config(?:\/-?\d+)?$/.test(normalizedPath)) return PAGE_OG_PREVIEWS.config
  if (normalizedPath === "/blocks") return PAGE_OG_PREVIEWS.blocks
  if (normalizedPath === "/abi") return PAGE_OG_PREVIEWS.abi
  if (normalizedPath === "/sources") return PAGE_OG_PREVIEWS.sources
  if (normalizedPath === "/faucet") return PAGE_OG_PREVIEWS.faucet
  if (normalizedPath === "/verified") return PAGE_OG_PREVIEWS.verified
  if (normalizedPath === "/verified/statistics") return PAGE_OG_PREVIEWS["verified-statistics"]
  if (/^\/verified\/[^/]+$/.test(normalizedPath)) return PAGE_OG_PREVIEWS["verified-contract"]
  if (normalizedPath === "/cell") return PAGE_OG_PREVIEWS.cell
  if (normalizedPath === "/emulate") return PAGE_OG_PREVIEWS.emulate
  if (normalizedPath === "/favorites") return PAGE_OG_PREVIEWS.favorites
  if (normalizedPath === "/suspended") return PAGE_OG_PREVIEWS.suspended
  if (/^\/block\/-?\d+\/[^/]+\/\d+$/.test(normalizedPath)) return PAGE_OG_PREVIEWS.block
  if (/^\/tx\/[^/]+(?:\/trace)?$/.test(normalizedPath)) return PAGE_OG_PREVIEWS.transaction
  return undefined
}

export function PageOgImage({preview}: {readonly preview: PageOgPreview}) {
  const accountStylePreview: AccountOgPreview = {
    title: preview.title,
    subtitle: preview.badge,
    shortAddress: "",
    rawAddress: "",
    type: preview.badge,
    detail: preview.description,
    detailLines: 2,
    avatarText: "",
  }

  return <AccountOgImage preview={accountStylePreview} variant="page" />
}

function normalizePath(pathname: string) {
  if (pathname === "/") return pathname
  return pathname.replace(/\/+$/, "") || "/"
}
