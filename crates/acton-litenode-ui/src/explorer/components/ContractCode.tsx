import {Buffer} from "node:buffer"

import type React from "react"
import {useMemo, useState} from "react"
import {Cell as Cell2, runtime, text} from "ton-assembly"

import styles from "./ContractCode.module.css"

interface ContractCodeProps {
  readonly codeBoc: string
}

type CodeTab = "decompiled" | "base64" | "hex"

export const ContractCode: React.FC<ContractCodeProps> = ({codeBoc}) => {
  const [activeTab, setActiveTab] = useState<CodeTab>("decompiled")

  const codeData = useMemo(() => {
    if (!codeBoc) return
    try {
      const buf = Buffer.from(codeBoc, "base64")
      const cell = Cell2.fromBoc(buf)[0]
      const decompiled = text.print(runtime.decompileCell(cell))

      return {
        base64: codeBoc,
        hex: Buffer.from(codeBoc, "base64").toString("hex").toUpperCase(),
        decompiled: decompiled,
      }
    } catch (error) {
      console.error("Failed to process contract code:", error)
      return {
        base64: codeBoc,
        hex: "Error processing HEX",
        decompiled: "Error: Failed to decompile code.",
      }
    }
  }, [codeBoc])

  if (!codeBoc || !codeData) {
    return (
      <div className={styles.container}>
        <div className={styles.empty}>No code available for this account.</div>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <div className={styles.tabs}>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "decompiled" ? styles.tabActive : ""}`}
          onClick={() => setActiveTab("decompiled")}
        >
          Decompiled
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "base64" ? styles.tabActive : ""}`}
          onClick={() => setActiveTab("base64")}
        >
          Base64
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "hex" ? styles.tabActive : ""}`}
          onClick={() => setActiveTab("hex")}
        >
          HEX
        </button>
      </div>

      <div className={styles.codeBlock}>
        <pre className={styles.code}>
          <code>{codeData[activeTab]}</code>
        </pre>
      </div>
    </div>
  )
}
