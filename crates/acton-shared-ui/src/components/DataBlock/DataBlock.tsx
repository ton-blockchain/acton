import type React from "react"

import styles from "./DataBlock.module.css"

interface DataBlockProps {
  readonly data: string
  readonly className?: string
  readonly visualDynamic?: string
  readonly visualPlaceholder?: string
}

export const DataBlock: React.FC<DataBlockProps> = ({
  data,
  className,
  visualDynamic,
  visualPlaceholder,
}) => {
  return (
    <div
      className={`${styles.container} ${className ?? ""}`}
      data-visual-dynamic={visualDynamic}
      data-visual-placeholder={visualPlaceholder}
    >
      {data}
    </div>
  )
}
