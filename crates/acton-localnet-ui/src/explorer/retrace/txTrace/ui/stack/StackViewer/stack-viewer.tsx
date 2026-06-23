import React, {type JSX, memo, useState} from "react"
import {type StackElement} from "ton-assembly/dist/trace"
import {Cell} from "@ton/core"
import {motion, AnimatePresence} from "framer-motion"

import {CopyValueButton} from "@acton/shared-ui"

import styles from "./StackViewer.module.css"

const truncateMiddle = (text: string, maxLength: number = 30): JSX.Element => {
  if (text.length <= maxLength) return <>{text}</>

  const partLength = Math.floor(maxLength / 2)
  const start = text.slice(0, partLength)
  const end = text.slice(text.length - partLength)

  return (
    <span title={text} className={styles.truncatedMiddle}>
      {start}
      <span className={styles.ellipsis}>…</span>
      {end}
    </span>
  )
}

interface StackViewerProps {
  readonly stack: readonly StackElement[]
  readonly title?: string
  readonly onStackItemClick?: (element: StackElement, title: string) => void
}

const getElementKey = (element: StackElement, index: number): string => {
  switch (element.$) {
    case "Integer":
      return `int-${element.value}-${index}`
    case "Cell":
      return `cell-${element.boc}-${index}`
    case "Slice":
      return `slice-${element.hex}-${element.startBit}-${element.startRef}-${index}`
    case "Builder":
      return `builder-${element.hex}-${index}`
    case "Continuation":
      return `cont-${element.name}-${index}`
    case "Address":
      return `addr-${element.value}-${index}`
    case "Tuple":
      return `tuple-${index}`
    case "Null":
      return `null-${index}`
    case "NaN":
      return `nan-${index}`
    case "Unknown":
      return `unknown-${index}`
    default:
      return `unknown-fallback-${index}`
  }
}

const safeCellFromHex = (boc: string) => {
  try {
    return Cell.fromHex(boc)
  } catch {
    return new Cell()
  }
}

const safeLoadAddress = (cell: Cell) => {
  try {
    return cell.asSlice().loadAddress()
  } catch {
    return undefined
  }
}

const StackViewer: React.FC<StackViewerProps> = ({stack, title, onStackItemClick}) => {
  const [expandedItem, setExpandedItem] = useState<string | null>(null)

  const toggleExpand = (key: string) => {
    setExpandedItem(prev => (prev === key ? null : key))
  }

  const handleOpenDetailsModal = (itemData: StackElement, elementTitle: string = "Stack Item") => {
    if (onStackItemClick) {
      onStackItemClick(itemData, elementTitle)
    } else {
      setExpandedItem(null)
    }
  }

  const renderStackElement = (element: StackElement, keyPrefix: string): JSX.Element => {
    const handleItemClick = () => {
      switch (element.$) {
        case "Cell":
          handleOpenDetailsModal(element, "Cell Details")
          break
        case "Slice":
          handleOpenDetailsModal(element, "Slice Details")
          break
        case "Builder":
          handleOpenDetailsModal(element, "Builder Details")
          break
        case "Address":
          handleOpenDetailsModal(element, "Address Details")
          break
        default:
          toggleExpand(keyPrefix)
      }
    }

    const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Enter" || event.key === " ") {
        handleItemClick()
      }
    }

    switch (element.$) {
      case "Null":
        return (
          <div className={styles.nullItem} key={keyPrefix}>
            null
          </div>
        )
      case "NaN":
        return (
          <div className={styles.nanItem} key={keyPrefix}>
            NaN
          </div>
        )
      case "Integer": {
        const value = element.value.toString()
        const hexPresentation =
          element.value < 0
            ? `-0x${(-element.value).toString(16)}`
            : `0x${element.value.toString(16)}`
        return (
          <div className={styles.integerItem} key={keyPrefix}>
            {value} <span className={styles.integerItemHexValue}>({hexPresentation})</span>
            <CopyValueButton
              className={styles.integerItemCopyButton}
              label="integer value"
              value={value.toString()}
            />
          </div>
        )
      }
      case "Cell": {
        const cell = safeCellFromHex(element.boc)
        return (
          <div
            className={styles.cellItem}
            key={keyPrefix}
            onClick={handleItemClick}
            onKeyDown={handleKeyDown}
            role="button"
            tabIndex={0}
          >
            <div className={styles.stackItemLabel}>Cell</div>
            <div className={styles.stackItemValue}>
              {cell.bits.length === 0 && cell.refs.length === 0
                ? "Empty Cell"
                : expandedItem === keyPrefix
                  ? element.boc
                  : truncateMiddle(element.boc, 35)}
              <div className={styles.stackItemDetails}>
                Bits: {cell.bits.length}, Refs: {cell.refs.length}
              </div>
              <CopyValueButton
                className={styles.cellItemCopyButton}
                label="cell as BoC"
                value={element.boc}
              />
            </div>
          </div>
        )
      }
      case "Slice": {
        const cell = safeCellFromHex(element.hex)

        if (cell.bits.length === 267 && cell.refs.length === 0) {
          const address = safeLoadAddress(cell)
          if (address) {
            const string = address.toRawString()
            const readable = address.toString()
            return (
              <div
                className={styles.addressItem}
                key={keyPrefix}
                onClick={() => handleOpenDetailsModal(element, "Address Details")}
                onKeyDown={e => {
                  if (e.key === "Enter" || e.key === " ")
                    handleOpenDetailsModal(element, "Address Details")
                }}
                role="button"
                tabIndex={0}
              >
                <div className={styles.stackItemLabel}>Address</div>
                <div className={styles.stackItemValue}>
                  {expandedItem === keyPrefix ? string : truncateMiddle(string, 35)}
                </div>
                <CopyValueButton
                  className={styles.addressItemCopyButton}
                  label="address as base64"
                  value={readable}
                />
              </div>
            )
          }
        }

        return (
          <div
            className={styles.sliceItem}
            key={keyPrefix}
            onClick={handleItemClick}
            onKeyDown={handleKeyDown}
            role="button"
            tabIndex={0}
          >
            <div className={styles.stackItemLabel}>Slice</div>
            <div className={styles.stackItemValue}>
              {cell.bits.length === 0 && cell.refs.length === 0
                ? "Empty Slice"
                : expandedItem === keyPrefix
                  ? element.hex
                  : truncateMiddle(element.hex, 35)}
              <CopyValueButton
                className={styles.sliceItemCopyButton}
                label="slice as BoC"
                value={element.hex}
              />
            </div>
            <div className={styles.stackItemDetails}>
              Bits: {element.startBit}-{cell.bits.length}, Refs: {element.startRef}-
              {cell.refs.length}
            </div>
          </div>
        )
      }
      case "Builder": {
        const cell = safeCellFromHex(element.hex)
        return (
          <div
            className={styles.builderItem}
            key={keyPrefix}
            onClick={handleItemClick}
            onKeyDown={handleKeyDown}
            role="button"
            tabIndex={0}
          >
            <div className={styles.stackItemLabel}>Builder</div>
            <div className={styles.stackItemValue}>
              {cell.bits.length === 0 && cell.refs.length === 0
                ? "Empty Builder"
                : expandedItem === keyPrefix
                  ? element.hex
                  : truncateMiddle(element.hex, 35)}
            </div>
            <div className={styles.stackItemDetails}>
              Bits: {cell.bits.length}, Refs: {cell.refs.length}
            </div>
            <CopyValueButton
              className={styles.builderItemCopyButton}
              label="builder as BoC"
              value={element.hex}
            />
          </div>
        )
      }
      case "Continuation": {
        return (
          <div
            className={styles.continuationItem}
            key={keyPrefix}
            onClick={() => toggleExpand(keyPrefix)}
            onKeyDown={e => {
              if (e.key === "Enter" || e.key === " ") toggleExpand(keyPrefix)
            }}
            role="button"
            tabIndex={0}
          >
            <div className={styles.stackItemLabel}>Continuation</div>
            <div className={styles.stackItemValue}>
              {expandedItem === keyPrefix ? element.name : truncateMiddle(element.name, 35)}
            </div>
            <CopyValueButton
              className={styles.continuationItemCopyButton}
              label="continuation"
              value={element.name}
            />
            {expandedItem === keyPrefix && (
              <div className={styles.stackItemFullview}>
                <button
                  type="button"
                  className={styles.closeBtn}
                  onClick={e => {
                    e.stopPropagation()
                    setExpandedItem(null)
                  }}
                >
                  Collapse
                </button>
              </div>
            )}
          </div>
        )
      }
      case "Address": {
        return (
          <div
            className={styles.addressItem}
            key={keyPrefix}
            onClick={handleItemClick}
            onKeyDown={handleKeyDown}
            role="button"
            tabIndex={0}
          >
            <div className={styles.stackItemLabel}>Address</div>
            <div className={styles.stackItemValue}>
              {expandedItem === keyPrefix ? element.value : truncateMiddle(element.value, 35)}
            </div>
            <CopyValueButton
              className={styles.addressItemCopyButton}
              label="address as base64"
              value={element.value}
            />
          </div>
        )
      }
      case "Tuple":
        return (
          <div className={styles.tupleItem} key={keyPrefix}>
            <div className={styles.stackItemLabel}>Tuple</div>
            <div className={styles.stackItems}>
              {element.elements.map((el, i) => {
                const nestedKeyPrefix = `${keyPrefix}-${i}`
                return (
                  <div className={styles.tupleElement} key={nestedKeyPrefix}>
                    {renderStackElement(el, nestedKeyPrefix)}
                  </div>
                )
              })}
            </div>
          </div>
        )
      case "Unknown":
        return (
          <div className={styles.unknownItem} key={keyPrefix} role="button" tabIndex={0}>
            <div className={styles.stackItemLabel}>Unknown</div>
            <div className={styles.stackItemValue}>
              {expandedItem === keyPrefix ? element.value : truncateMiddle(element.value, 35)}
            </div>
          </div>
        )
      default:
        return (
          <div className={styles.stackItem} key={keyPrefix}>
            Unknown element type
          </div>
        )
    }
  }

  const itemVariants = {
    initial: {opacity: 0, y: 20},
    animate: {opacity: 1, y: 0},
    exit: {opacity: 0, y: -20},
  }

  const itemsToRender = stack.map((el, index) => ({
    element: el,
    key: getElementKey(el, index),
    originalIndex: stack.length - 1 - index,
  }))

  return (
    <div className={styles.stackViewer}>
      {title && <h3 className={styles.stackTitle}>{title}</h3>}
      <div className={styles.stackContainer}>
        {itemsToRender.length === 0 ? (
          <div className={styles.emptyStack}>Empty stack</div>
        ) : (
          <div className={styles.stackItems}>
            <AnimatePresence mode="popLayout">
              {[...itemsToRender].reverse().map(({element, key, originalIndex}) => (
                <motion.div
                  key={key}
                  layout
                  variants={itemVariants}
                  initial="initial"
                  animate="animate"
                  exit="exit"
                  transition={{type: "spring", stiffness: 300, damping: 30}}
                  className={styles.stackElement}
                >
                  <div className={styles.stackIndex}>{originalIndex}</div>
                  {renderStackElement(element, key)}
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>
    </div>
  )
}

export default memo(StackViewer)
