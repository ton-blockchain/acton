import { ChevronRight } from "lucide-react"
import React, { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import styles from "./Breadcrumbs.module.css"
import { fetchAddressName, formatAddress } from "./utils"

export interface BreadcrumbItem {
  label: string
  path?: string
  isAddress?: boolean
  isHash?: boolean
}

interface BreadcrumbsProps {
  items: BreadcrumbItem[]
}

export const Breadcrumbs: React.FC<BreadcrumbsProps> = ({ items }) => {
  const [labels, setLabels] = useState<Record<number, string>>({})

  useEffect(() => {
    let isActive = true
    items.forEach((item, index) => {
      if (item.isAddress) {
        fetchAddressName(item.label).then((name) => {
          if (!isActive || !name) return
          setLabels((prev) => {
            if (prev[index] === name) return prev
            return { ...prev, [index]: name }
          })
        })
      }
    })
    return () => {
      isActive = false
    }
  }, [items])

  const formatItem = (item: BreadcrumbItem, index: number) => {
    if (item.isAddress) {
      return labels[index] || formatAddress(item.label)
    }
    if (item.isHash) {
      return formatAddress(item.label)
    }
    return item.label
  }

  return (
    <nav className={styles.breadcrumbs}>
      <Link to="/explorer" className={styles.item}>
        <span>Explore</span>
      </Link>
      {items.map((item, index) => {
        const key = `${item.label}-${index}`
        return (
          <React.Fragment key={key}>
            <ChevronRight size={14} className={styles.separator} />
            {item.path ? (
              <Link to={item.path} className={styles.item}>
                {formatItem(item, index)}
              </Link>
            ) : (
              <span className={`${styles.item} ${styles.current}`}>{formatItem(item, index)}</span>
            )}
          </React.Fragment>
        )
      })}
    </nav>
  )
}
