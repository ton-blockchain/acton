import type React from "react"

import styles from "./DataBlock.module.css"

interface DataBlockProps {
  readonly data: string
  readonly className?: string
}

export const DataBlock: React.FC<DataBlockProps> = ({data, className}) => {
  return <div className={`${styles.container} ${className ?? ""}`}>{data}</div>
}
