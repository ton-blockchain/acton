import {Breadcrumbs, type BreadcrumbsItem, CopyInlineAction, InlineActions} from "@acton/ui"
import {Link} from "react-router"
import type {FC, ReactNode} from "react"

import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {ExplorerAddressChip} from "./ExplorerAddressChip"
import styles from "./ExplorerBreadcrumbs.module.css"
import {formatAddress} from "./utils"

export interface ExplorerBreadcrumbItem {
  readonly label: string
  readonly path?: string
  readonly isAddress?: boolean
  readonly isHash?: boolean
  readonly copy?: Readonly<{
    readonly value: string
    readonly label: string
    readonly copiedLabel: string
  }>
}

interface ExplorerBreadcrumbsProps {
  readonly items: ExplorerBreadcrumbItem[]
}

type BreadcrumbLink = NonNullable<BreadcrumbsItem["link"]>

function createBreadcrumbLink(path: string): BreadcrumbLink {
  return (children, className) => (
    <Link to={path} className={className}>
      {children}
    </Link>
  )
}

function formatItem(item: ExplorerBreadcrumbItem): ReactNode {
  const label = item.isAddress ? (
    <ExplorerAddressChip
      address={item.label}
      className={styles.address}
      copyable={false}
      variant="plain"
    />
  ) : item.isHash ? (
    formatAddress(item.label)
  ) : (
    item.label
  )

  if (item.copy) {
    return (
      <InlineActions
        visibility="hover"
        actions={
          <CopyInlineAction
            value={item.copy.value}
            label={item.copy.label}
            copiedLabel={item.copy.copiedLabel}
          />
        }
      >
        {label}
      </InlineActions>
    )
  }

  return label
}

export const ExplorerBreadcrumbs: FC<ExplorerBreadcrumbsProps> = ({items}) => {
  const routes = useExplorerRoutePaths()
  const breadcrumbItems: BreadcrumbsItem[] = [
    {
      id: "explore",
      label: "Explore",
      truncate: false,
      link: createBreadcrumbLink(routes.rootPath),
    },
    ...items.map((item, index) => {
      const path = item.path
      return {
        id: `${item.label}-${index}`,
        label: formatItem(item),
        link: path ? createBreadcrumbLink(path) : undefined,
      }
    }),
  ]

  return (
    <Breadcrumbs
      ariaLabel="Explorer breadcrumb"
      className={styles.breadcrumbs}
      items={breadcrumbItems}
    />
  )
}
