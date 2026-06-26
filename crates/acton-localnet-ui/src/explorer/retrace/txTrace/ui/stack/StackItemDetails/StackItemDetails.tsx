import React from "react"

import {type StackElement} from "@ton/tasm/dist/trace"
import {Cell} from "@ton/core"

import {DataBlock} from "@acton/shared-ui"

import AddressDetails from "../AddressDetails"
import CellTreeView from "../CellTreeView/CellTreeView"
import styles from "./StackItemDetails.module.css"

interface StackItemDetailsProps {
  readonly itemData: StackElement | null
  readonly title?: string
  readonly onClose?: () => void
}

const StackItemDetails: React.FC<StackItemDetailsProps> = ({itemData, title, onClose}) => {
  if (!itemData) {
    return (
      <div className={styles.detailsContainer}>
        <p>No item selected</p>
      </div>
    )
  }

  let cellDetailsContent: React.ReactNode | null = null
  let treeViewContent: React.ReactNode | null = null

  const cellFromItem = (itemData: StackElement) => {
    if (itemData.$ === "Cell" && itemData?.boc) {
      return Cell.fromHex(itemData.boc)
    }
    if ((itemData.$ === "Slice" || itemData.$ === "Builder") && itemData?.hex) {
      return Cell.fromHex(itemData.hex)
    }
    return null
  }

  const safeLoadAddress = (cell: Cell) => {
    try {
      return cell.asSlice().loadAddress()
    } catch {
      return undefined
    }
  }

  try {
    const rootCell = cellFromItem(itemData)
    if (rootCell) {
      if (rootCell.bits.length === 267 && rootCell.refs.length === 0) {
        const address = safeLoadAddress(rootCell)
        if (address) {
          cellDetailsContent = <AddressDetails address={address} />
          treeViewContent = null
        } else {
          cellDetailsContent = (
            <>
              <div className={styles.dataSection}>
                <DataBlock data={rootCell.toBoc().toString("hex")} maxHeight={100} />
              </div>
            </>
          )
          treeViewContent = <CellTreeView cell={rootCell} />
        }
      } else {
        cellDetailsContent = (
          <>
            <div className={styles.dataSection}>
              <DataBlock data={rootCell.toBoc().toString("hex")} maxHeight={100} />
            </div>
          </>
        )
        treeViewContent = <CellTreeView cell={rootCell} />
      }
    } else {
      treeViewContent = null
      cellDetailsContent = <p>Details for the selected item will be shown here.</p>
    }
  } catch (error) {
    console.error("Error processing item data:", error)
    treeViewContent = null
    cellDetailsContent = <p>Error displaying item details. Data might be malformed.</p>
  }

  return (
    <div className={styles.detailsContainer}>
      {(title || onClose) && (
        <div className={styles.header}>
          {title && <h3 className={styles.title}>{title}</h3>}
          {onClose && (
            <button
              type="button"
              onClick={onClose}
              className={styles.closeButton}
              aria-label="Close details"
            >
              ×
            </button>
          )}
        </div>
      )}
      <div className={styles.contentContainer}>
        <div className={styles.detailsRow}>{cellDetailsContent}</div>

        {treeViewContent && (
          <div className={styles.treeRow}>
            <div className={styles.treeViewContainer}>{treeViewContent}</div>
          </div>
        )}
      </div>
    </div>
  )
}

export default StackItemDetails
