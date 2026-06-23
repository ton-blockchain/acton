import React, {useState, useMemo} from "react"
import {Cell} from "@ton/core"

import styles from "./CellTreeView.module.css"

const CopyIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
  </svg>
)

const CheckIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <polyline points="20 6 9 17 4 12"></polyline>
  </svg>
)

const collectAllBitsLengths = (cell: Cell, lengths: number[] = []): number[] => {
  lengths.push(cell.bits.length)

  for (const refCell of cell.refs) {
    collectAllBitsLengths(refCell, lengths)
  }

  return lengths
}

const calculateCellWidth = (bitsLength: number, maxBitsLength: number): number => {
  if (maxBitsLength === 0) return 200

  const ratio = bitsLength / maxBitsLength
  const calculatedWidth = ratio * 400

  return Math.max(calculatedWidth, 60)
}

interface CellTreeViewProps {
  readonly cell: Cell
  readonly depth?: number
  readonly maxBitsLength?: number
}

const CellTreeView: React.FC<CellTreeViewProps> = ({cell, depth = 0, maxBitsLength}) => {
  const [isExpanded, setIsExpanded] = useState(true)
  const [copied, setCopied] = useState(false)

  const calculatedMaxBitsLength = useMemo(() => {
    if (maxBitsLength !== undefined) {
      return maxBitsLength
    }

    if (depth === 0) {
      const allLengths = collectAllBitsLengths(cell)
      return Math.max(...allLengths)
    }

    return 0
  }, [cell, depth, maxBitsLength])

  const refsCount = cell.refs.length
  const cellWidth = calculateCellWidth(cell.bits.length, calculatedMaxBitsLength)

  if (depth > 10) {
    return (
      <div className={styles.cellNode} style={{marginLeft: 10}}>
        Max depth reached.
      </div>
    )
  }

  const toggleExpand = (e: React.MouseEvent | React.KeyboardEvent) => {
    e.stopPropagation()
    if (refsCount > 0) {
      setIsExpanded(!isExpanded)
    }
  }

  const handleCopyToClipboard = (e: React.MouseEvent) => {
    e.stopPropagation()
    const bocHex = cell.toBoc().toString("hex")
    navigator.clipboard
      .writeText(bocHex)
      .then(() => {
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      })
      .catch(err => {
        console.error("Failed to copy BoC: ", err)
        setTimeout(() => setCopied(false), 1500)
      })
  }

  const cellInfoClassName = `${styles.cellInfo} ${refsCount > 0 ? styles.cellInfoClickable : ""}`
  const cellNodeColorClass = styles[`cellNodeColor${(depth % 5) + 1}`]

  return (
    <div className={`${styles.cellNode}`} style={{marginLeft: depth * 5}}>
      <div
        className={`${cellInfoClassName} ${cellNodeColorClass}`}
        style={{width: cellWidth}}
        onClick={refsCount > 0 ? toggleExpand : undefined}
        onKeyDown={
          refsCount > 0
            ? (e: React.KeyboardEvent) => {
                if (e.key === "Enter" || e.key === " ") toggleExpand(e)
              }
            : undefined
        }
        role={refsCount > 0 ? "button" : undefined}
        tabIndex={refsCount > 0 ? 0 : undefined}
      >
        <div className={styles.cellTextContent}>
          <strong>Bits:</strong> {cell.bits.length}
          <br />
          <strong>Refs:</strong>{" "}
          {refsCount > 0 ? (
            <span className={styles.refsToggle}>
              {isExpanded ? "▾" : "▸"} {refsCount}
            </span>
          ) : (
            refsCount
          )}
        </div>
        <button
          className={`${styles.copyButton} ${copied ? styles.copied : ""}`}
          onClick={handleCopyToClipboard}
          title="Copy BoC (Hex)"
        >
          {copied ? <CheckIcon /> : <CopyIcon />}
        </button>
      </div>
      {isExpanded && refsCount > 0 && (
        <div className={styles.cellRefs}>
          {cell.refs.map((refCell, index) => (
            <CellTreeView
              key={`${cell.hash().toString("hex")}-ref-${index}`}
              cell={refCell}
              depth={depth + 1}
              maxBitsLength={calculatedMaxBitsLength}
            />
          ))}
        </div>
      )}
    </div>
  )
}

export default CellTreeView
