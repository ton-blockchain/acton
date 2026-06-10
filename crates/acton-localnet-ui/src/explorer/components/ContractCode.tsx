import {Buffer} from "node:buffer"

import {decodeStorageDataCell, ParsedValueView} from "@acton/shared-ui"
import type {ContractData} from "@acton/shared-ui"
import type React from "react"
import {useEffect, useMemo, useState} from "react"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Cell as Cell2, runtime, text} from "ton-assembly"

import styles from "./ContractCode.module.css"

interface ContractCodeProps {
  readonly codeBoc: string
  readonly dataBoc?: string
  readonly compilerAbi?: ContractABI
  readonly compilerAbiLoading?: boolean
  readonly compilerAbiError?: string
  readonly onContractClick?: (address: string) => void
}

type CodeTab = "decompiled" | "storage" | "abi" | "base64" | "hex"

export const ContractCode: React.FC<ContractCodeProps> = ({
  codeBoc,
  dataBoc,
  compilerAbi,
  compilerAbiLoading = false,
  compilerAbiError,
  onContractClick,
}) => {
  const [activeTab, setActiveTab] = useState<CodeTab>("decompiled")

  useEffect(() => {
    if (!compilerAbi && activeTab === "storage") {
      setActiveTab("decompiled")
    }
  }, [activeTab, compilerAbi])

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

  const parsedStorage = useMemo(
    () => decodeStorageDataCell(dataBoc, compilerAbi),
    [dataBoc, compilerAbi],
  )
  const contracts = useMemo(() => new Map<string, ContractData>(), [])
  const abiJson = useMemo(() => {
    if (!compilerAbi) return
    return JSON.stringify(compilerAbi, undefined, 2)
  }, [compilerAbi])

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
        {compilerAbi && (
          <button
            type="button"
            className={`${styles.tab} ${activeTab === "storage" ? styles.tabActive : ""}`}
            onClick={() => setActiveTab("storage")}
          >
            Storage
          </button>
        )}
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "abi" ? styles.tabActive : ""}`}
          onClick={() => setActiveTab("abi")}
        >
          ABI
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
        {activeTab === "storage" ? (
          parsedStorage ? (
            <div className={styles.storageBlock}>
              <ParsedValueView
                value={parsedStorage.value}
                contracts={contracts}
                onContractClick={onContractClick}
                fallbackTypeName={parsedStorage.name}
              />
            </div>
          ) : (
            <div className={styles.empty}>
              {dataBoc
                ? "Storage data could not be decoded with this ABI."
                : "No storage data available for this account."}
            </div>
          )
        ) : activeTab === "abi" ? (
          compilerAbiError ? (
            <div className={styles.empty}>Failed to load compiler ABI: {compilerAbiError}</div>
          ) : compilerAbiLoading ? (
            <div className={styles.empty}>Loading compiler ABI...</div>
          ) : abiJson ? (
            <pre className={styles.code}>
              <code>{abiJson}</code>
            </pre>
          ) : (
            <div className={styles.empty}>No compiler ABI registered for this contract.</div>
          )
        ) : (
          <pre
            className={`${styles.code} ${
              activeTab === "base64" || activeTab === "hex" ? styles.codeWrap : ""
            }`}
          >
            <code>{codeData[activeTab]}</code>
          </pre>
        )}
      </div>
    </div>
  )
}
