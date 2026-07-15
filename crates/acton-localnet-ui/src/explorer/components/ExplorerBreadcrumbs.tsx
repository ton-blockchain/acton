import {Breadcrumbs, type BreadcrumbsItem} from "@acton/ui"
import {Link} from "react-router-dom"
import type {FC, ReactNode} from "react"

import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {AddressLabel} from "./AddressLabel"
import styles from "./ExplorerBreadcrumbs.module.css"
import {formatAddress} from "./utils"

export interface ExplorerBreadcrumbItem {
  readonly label: string
  readonly path?: string
  readonly isAddress?: boolean
  readonly isHash?: boolean
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
  if (item.isAddress) {
    return <AddressLabel address={item.label} />
  }
  if (item.isHash) {
    return formatAddress(item.label)
  }
  return item.label
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
