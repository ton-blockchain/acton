import {useEffect, useMemo} from "react"
import {HighlightedCode, RawDataBlock} from "@acton/ui"
import {DynamicCtx, type ContractABI} from "@ton/tolk-abi-to-typescript"

import {AbiReadonlyGetMethodsSection} from "./AbiGetMethods"
import {
  AbiDeclarationsSection,
  AbiMessagesSection,
  AbiStorageSection,
  AbiThrownErrorsSection,
} from "./abiSections"
import {scrollToAbiSymbol} from "./abiShared"
import styles from "./AbiViewer.module.css"

export type AbiTab = "view" | "raw"

export interface AbiPanelProps {
  readonly activeTab: AbiTab
  readonly onTabChange: (tab: AbiTab) => void
  readonly abi: ContractABI
  readonly heightMode?: "contained" | "content"
  readonly showSymbolAnchors?: boolean
}

export function AbiPanel({
  activeTab,
  onTabChange,
  abi,
  heightMode = "contained",
  showSymbolAnchors = false,
}: AbiPanelProps) {
  const abiJson = useMemo(() => JSON.stringify(abi, undefined, 2), [abi])
  const rootClassName = [styles.shell, heightMode === "content" ? styles.shellContent : ""]
    .filter(Boolean)
    .join(" ")

  useEffect(() => {
    const scrollToCurrentHashSymbol = () => {
      if (!globalThis.location.hash) return

      const id = decodeURIComponent(globalThis.location.hash.slice(1))
      if (!id.startsWith("abi-")) return

      const target = globalThis.document.getElementById(id)
      if (target instanceof HTMLDetailsElement) target.open = true
      scrollToAbiSymbol(target)
    }
    const openCurrentHashSymbol = () => {
      scrollToCurrentHashSymbol()
      globalThis.requestAnimationFrame(scrollToCurrentHashSymbol)
      globalThis.setTimeout(scrollToCurrentHashSymbol, 80)
    }

    openCurrentHashSymbol()
    globalThis.addEventListener("hashchange", openCurrentHashSymbol)
    return () => globalThis.removeEventListener("hashchange", openCurrentHashSymbol)
  }, [abi])

  return (
    <section className={rootClassName}>
      <div className={styles.tabBar}>
        {(
          [
            {tab: "view", label: "Rendered"},
            {tab: "raw", label: "Raw JSON"},
          ] as const
        ).map(item => (
          <button
            key={item.tab}
            type="button"
            className={`${styles.tab} ${activeTab === item.tab ? styles.tabActive : ""}`}
            onClick={() => onTabChange(item.tab)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {activeTab === "view" ? (
        <AbiView abi={abi} showSymbolAnchors={showSymbolAnchors} />
      ) : (
        <RawDataBlock
          className={styles.rawBlock}
          contentClassName={heightMode === "content" ? styles.rawBlockContent : undefined}
          variant="standalone"
          value={abiJson}
          copyLabel="ABI"
          customContent={
            <HighlightedCode className={styles.highlightedCode} value={abiJson} language="json" />
          }
        />
      )}
    </section>
  )
}

function AbiView({
  abi,
  showSymbolAnchors,
}: {
  readonly abi: ContractABI
  readonly showSymbolAnchors: boolean
}) {
  const ctx = useMemo(() => new DynamicCtx(abi), [abi])
  const symbols = ctx.symbols

  return (
    <section className={`${styles.dataPanel} ${styles.panel}`}>
      <div className={styles.view}>
        <header className={styles.header}>
          <div>
            <h3 className={styles.title}>{abi.contract_name}</h3>
            <div className={styles.facts} aria-label="Compiler ABI metadata">
              <span>
                <strong>Compiler:</strong> {abi.compiler_name} {abi.compiler_version}
              </span>
              {abi.version && (
                <span>
                  <strong>Contract version:</strong> v{abi.version}
                </span>
              )}
              {abi.author && (
                <span>
                  <strong>Author:</strong> {abi.author}
                </span>
              )}
            </div>
            {abi.description && <p className={styles.description}>{abi.description}</p>}
          </div>
        </header>

        <AbiReadonlyGetMethodsSection
          methods={abi.get_methods}
          symbols={symbols}
          showSymbolAnchors={showSymbolAnchors}
        />
        <AbiMessagesSection abi={abi} symbols={symbols} showSymbolAnchors={showSymbolAnchors} />
        <AbiStorageSection
          storage={abi.storage}
          symbols={symbols}
          showSymbolAnchors={showSymbolAnchors}
        />
        <AbiDeclarationsSection
          declarations={abi.declarations}
          symbols={symbols}
          showSymbolAnchors={showSymbolAnchors}
        />
        <AbiThrownErrorsSection errors={abi.thrown_errors} showSymbolAnchors={showSymbolAnchors} />
      </div>
    </section>
  )
}
