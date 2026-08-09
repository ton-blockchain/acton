import {Breadcrumbs, type BreadcrumbsItem} from "@acton/ui"
import type {ReactNode} from "react"

import styles from "./breadcrumbsGallery.module.css"
import type {ComponentGallery} from "./types"

const link = (href: string) => (children: ReactNode, className: string) => (
  <a href={href} className={className} onClick={event => event.preventDefault()}>
    {children}
  </a>
)

function BasicPath() {
  const items = [
    {label: "Explore", link: link("/explore"), truncate: false},
    {label: "Blocks", link: link("/explore/blocks"), truncate: false},
    {label: "Masterchain block 48169205", current: true},
  ] satisfies readonly BreadcrumbsItem[]

  return <Breadcrumbs items={items} />
}

function LongTechnicalPath() {
  const items = [
    {label: "Explore", link: link("/explore"), truncate: false},
    {
      label: "EQB8YtZZA7Kzz3cH8B36fb5FTB2v3Gj1eSk6fDrkqYGFN4q8",
      link: link("/explore/address/EQB8YtZZA7Kzz3cH8B36fb5FTB2v3Gj1eSk6fDrkqYGFN4q8"),
      preserveStart: 10,
      preserveEnd: 8,
      truncate: "middle",
    },
    {
      label: "65a184650d89a7a435714780a2f6084a8b1c11180c76672cc54f5c6412a23fc0",
      current: true,
      preserveStart: 10,
      preserveEnd: 10,
      truncate: "middle",
    },
  ] satisfies readonly BreadcrumbsItem[]

  return (
    <div className={styles.narrowFrame}>
      <Breadcrumbs items={items} />
    </div>
  )
}

function PartialLoadingPath() {
  const items = [
    {label: "Explore", link: link("/explore"), truncate: false},
    {
      label: "EQB8YtZZA7Kzz3cH8B36fb5FTB2v3Gj1eSk6fDrkqYGFN4q8",
      link: link("/explore/address/EQB8YtZZA7Kzz3cH8B36fb5FTB2v3Gj1eSk6fDrkqYGFN4q8"),
      preserveStart: 10,
      preserveEnd: 8,
      truncate: "middle",
    },
    {
      loading: true,
      loadingLabel: "Loading transaction hash",
      skeletonWidth: "14rem",
    },
  ] satisfies readonly BreadcrumbsItem[]

  return (
    <div className={styles.traceFrame}>
      <Breadcrumbs items={items} ariaLabel="Transaction trace path" />
    </div>
  )
}

function MultipleLoadingSegments() {
  const items = [
    {label: "Explore", link: link("/explore"), truncate: false},
    {label: "Accounts", link: link("/explore/accounts"), truncate: false},
    {
      loading: true,
      loadingLabel: "Loading account address",
      skeletonWidth: "14rem",
    },
    {
      loading: true,
      loadingLabel: "Loading transaction hash",
      skeletonWidth: "18rem",
    },
  ] satisfies readonly BreadcrumbsItem[]

  return <Breadcrumbs items={items} />
}

export const breadcrumbsGallery = {
  id: "breadcrumbs",
  title: "Breadcrumbs",
  status: "ready",
  summary:
    "Breadcrumbs renders compact explorer-style navigation paths with router-agnostic links, current item styling, and per-segment skeleton loading.",
  importStatement: 'import { Breadcrumbs } from "@acton/ui"',
  agentSummary:
    "Use Breadcrumbs for page hierarchy and trace paths. Pass router links through the item link callback and mark only unresolved segments as loading.",
  usage: [
    "Use for explorer page paths such as Explore > Blocks > Masterchain block.",
    "Use item.link to integrate react-router or another router without coupling the UI package.",
    "Use truncate: false for stable labels and truncate: middle for long addresses or hashes.",
    "Use item.loading with skeletonWidth when only part of the breadcrumb path is still loading.",
  ],
  avoid: [
    "Do not replace the whole breadcrumb row with a custom skeleton when stable path segments are already known.",
    "Do not put address or hash formatting logic inside Breadcrumbs.",
    "Do not use when the items are not ordered ancestors of the current page.",
  ],
  sections: [
    {
      id: "breadcrumbs-basic-path",
      title: "Basic Path",
      description: "Standard explorer hierarchy with a current final segment.",
      content: <BasicPath />,
    },
    {
      id: "breadcrumbs-long-technical-path",
      title: "Long Technical Path",
      description: "Address and hash segments truncate instead of breaking the row.",
      content: <LongTechnicalPath />,
    },
    {
      id: "breadcrumbs-partial-loading",
      title: "Partial Loading",
      description:
        "Known path segments stay visible while an unresolved trace segment is skeletonized.",
      content: <PartialLoadingPath />,
    },
    {
      id: "breadcrumbs-multiple-loading",
      title: "Multiple Loading Segments",
      description: "Several unresolved path segments can load independently.",
      content: <MultipleLoadingSegments />,
    },
  ],
} satisfies ComponentGallery
