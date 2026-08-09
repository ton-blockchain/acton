import {CheckCircle2, CircleSlash} from "lucide-react"

import styles from "./StatusPill.module.css"

interface StatusPillProps {
  readonly verified: boolean
}

export function StatusPill({verified}: StatusPillProps) {
  return (
    <span className={`${styles.pill} ${verified ? styles.verified : styles.unverified}`}>
      {verified ? <CheckCircle2 size={15} /> : <CircleSlash size={15} />}
      {verified ? "Verified" : "Not verified"}
    </span>
  )
}
