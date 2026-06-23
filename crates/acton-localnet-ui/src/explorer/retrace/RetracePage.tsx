import {useParams} from "react-router-dom"

import TracePage from "@retrace/pages/TracePage"

import "./RetracePage.tokens.css"
import styles from "./RetracePage.module.css"

export function RetracePage() {
  const {hash = ""} = useParams<{hash: string}>()

  return (
    <div className={`${styles.root} retraceRoot`}>
      <TracePage initialTx={hash} />
    </div>
  )
}
