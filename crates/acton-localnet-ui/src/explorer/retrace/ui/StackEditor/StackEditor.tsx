import React, {useState, useCallback, type JSX, useEffect, memo} from "react"
import {FiPlus, FiTrash2, FiArrowUp, FiArrowDown, FiFileText, FiCheck, FiX} from "react-icons/fi"
import {type StackElement} from "ton-assembly/dist/trace"
import {logs} from "ton-assembly"
import {Cell, Address, Builder} from "@ton/core"
import {motion, AnimatePresence} from "framer-motion"

import {RiFileCloseLine} from "react-icons/ri"

import Button from "@retrace/ui/Button"
import {CopyButton} from "@retrace/CopyButton/CopyButton"

import styles from "./StackEditor.module.css"

export interface StackEditorProps {
  readonly stack: StackElement[]
  readonly onStackChange: (stack: StackElement[]) => void
}

interface StackItemForm {
  readonly type: "Integer" | "Cell" | "Slice" | "Address" | "Null"
  readonly value: string
}

const truncateMiddle = (text: string, maxLength: number = 30): JSX.Element => {
  if (text.length <= maxLength) return <>{text}</>

  const partLength = Math.floor(maxLength / 2)
  const start = text.substring(0, partLength)
  const end = text.substring(text.length - partLength)

  return (
    <span title={text} className={styles.truncatedMiddle}>
      {start}
      <span className={styles.ellipsis}>…</span>
      {end}
    </span>
  )
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

const StackEditor: React.FC<StackEditorProps> = ({stack, onStackChange}) => {
  const [expandedItem, setExpandedItem] = useState<string | null>(null)
  const [showTextImport, setShowTextImport] = useState(false)
  const [textStackInput, setTextStackInput] = useState("")
  const [parseError, setParseError] = useState<string | null>(null)
  const [isAddingNewItem, setIsAddingNewItem] = useState(false)
  const [newItemForm, setNewItemForm] = useState<StackItemForm>({type: "Integer", value: ""})
  const [validationError, setValidationError] = useState<string | null>(null)

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "s") {
        event.preventDefault()
        event.stopPropagation()
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [])

  const toggleExpand = (key: string) => {
    setExpandedItem(prev => (prev === key ? null : key))
  }

  const removeStackItem = useCallback(
    (originalIndex: number) => {
      if (!Array.isArray(stack)) return
      // Convert originalIndex back to actual array index
      const actualIndex = stack.length - 1 - originalIndex
      const newStack = stack.filter((_, i) => i !== actualIndex)
      onStackChange(newStack)
    },
    [stack, onStackChange],
  )

  const moveStackItem = useCallback(
    (originalIndex: number, direction: "up" | "down") => {
      if (!Array.isArray(stack)) return
      const newStack = [...stack]
      // Convert originalIndex back to actual array index
      const actualIndex = stack.length - 1 - originalIndex
      // "up" means towards stack top (smaller originalIndex, larger actualIndex)
      // "down" means towards stack bottom (larger originalIndex, smaller actualIndex)
      const targetIndex = direction === "up" ? actualIndex + 1 : actualIndex - 1

      if (targetIndex >= 0 && targetIndex < newStack.length) {
        ;[newStack[actualIndex], newStack[targetIndex]] = [
          newStack[targetIndex],
          newStack[actualIndex],
        ]
        onStackChange(newStack)
      }
    },
    [stack, onStackChange],
  )

  const clearStack = useCallback(() => {
    onStackChange([])
  }, [onStackChange])

  const parseTextStack = useCallback(() => {
    if (!textStackInput.trim()) {
      setParseError("Please enter stack text")
      return
    }

    try {
      setParseError(null)
      const parsed = logs.parseStack(textStackInput.trim())
      if (!parsed) {
        setParseError("Could not parse stack text")
        return
      }

      const stackElements = logs.processStack(parsed)
      if (!Array.isArray(stackElements)) {
        setParseError("Invalid stack format")
        return
      }

      onStackChange(stackElements)
      setTextStackInput("")
      setShowTextImport(false)
    } catch (error) {
      setParseError(error instanceof Error ? error.message : "Parse error")
    }
  }, [textStackInput, onStackChange])

  const validateValue = useCallback((type: StackItemForm["type"], value: string): string | null => {
    if (type === "Null") return null
    if (!value.trim()) return null

    try {
      switch (type) {
        case "Integer":
          BigInt(value)
          break
        case "Cell":
          Cell.fromHex(value)
          break
        case "Slice":
          if (!/^[0-9a-fA-F]*$/.test(value)) {
            return "Invalid hex format"
          }
          break
        case "Address":
          Address.parse(value)
          break
      }
      return null
    } catch (error) {
      return error instanceof Error ? error.message : "Invalid value"
    }
  }, [])

  const handleStartAddingItem = useCallback(() => {
    setIsAddingNewItem(true)
    setNewItemForm({type: "Integer", value: ""})
    setValidationError(null)
    setShowTextImport(false)
  }, [])

  const handleCancelAddingItem = useCallback(() => {
    setIsAddingNewItem(false)
    setNewItemForm({type: "Integer", value: ""})
    setValidationError(null)
  }, [])

  const handleSaveNewItem = useCallback(() => {
    if (newItemForm.type !== "Null" && !newItemForm.value.trim()) {
      setValidationError("Value is required")
      return
    }

    const error = validateValue(newItemForm.type, newItemForm.value)
    if (error) {
      setValidationError(error)
      return
    }

    setValidationError(null)
    let stackElement: StackElement

    try {
      switch (newItemForm.type) {
        case "Integer":
          stackElement = {
            $: "Integer",
            value: BigInt(newItemForm.value),
          }
          break
        case "Cell":
          // Validate BoC hex
          Cell.fromHex(newItemForm.value)
          stackElement = {
            $: "Cell",
            boc: newItemForm.value,
          }
          break
        case "Slice":
          stackElement = {
            $: "Slice",
            hex: newItemForm.value,
            startBit: 0,
            endBit: 0,
            startRef: 0,
            endRef: 0,
          }
          break
        case "Address": {
          const address = Address.parse(newItemForm.value)
          const builder = new Builder()
          builder.storeAddress(address)
          const addressCell = builder.endCell()
          stackElement = {
            $: "Slice",
            hex: addressCell.toBoc().toString("hex"),
            startBit: 0,
            endBit: addressCell.bits.length,
            startRef: 0,
            endRef: addressCell.refs.length,
          }
          break
        }
        case "Null":
          stackElement = {
            $: "Null",
          }
          break
        default:
          return
      }

      onStackChange([...(Array.isArray(stack) ? stack : []), stackElement])
      setIsAddingNewItem(false)
      setNewItemForm({type: "Integer", value: ""})
      setValidationError(null)
    } catch (error) {
      setValidationError(error instanceof Error ? error.message : "Invalid value")
    }
  }, [newItemForm, stack, onStackChange, validateValue])

  const renderStackElement = (
    element: StackElement,
    keyPrefix: string,
    originalIndex: number,
  ): JSX.Element => {
    const handleItemClick = () => {
      toggleExpand(keyPrefix)
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
        return (
          <div className={styles.integerItem} key={keyPrefix}>
            {value}
            <CopyButton
              className={styles.integerItemCopyButton}
              title="Copy integer value"
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
                  : truncateMiddle(element.boc, 40)}
              <div className={styles.stackItemDetails}>
                Bits: {cell.bits.length}, Refs: {cell.refs.length}
              </div>
              <CopyButton
                className={styles.cellItemCopyButton}
                title="Copy cell as BoC"
                value={element.boc}
              />
            </div>
          </div>
        )
      }
      case "Slice": {
        const cell = safeCellFromHex(element.hex)
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
                  : truncateMiddle(element.hex, 40)}
              <CopyButton
                className={styles.sliceItemCopyButton}
                title="Copy slice as BoC"
                value={element.hex}
              />
            </div>
            <div className={styles.stackItemDetails}>
              Bits: {element.startBit}-{element.endBit}, Refs: {element.startRef}-{element.endRef}
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
                  : truncateMiddle(element.hex, 40)}
            </div>
            <div className={styles.stackItemDetails}>
              Bits: {cell.bits.length}, Refs: {cell.refs.length}
            </div>
            <CopyButton
              className={styles.builderItemCopyButton}
              title="Copy builder as BoC"
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
              {expandedItem === keyPrefix ? element.name : truncateMiddle(element.name, 40)}
            </div>
            <CopyButton
              className={styles.continuationItemCopyButton}
              title="Copy continuation"
              value={element.name}
            />
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
              {expandedItem === keyPrefix ? element.value : truncateMiddle(element.value, 40)}
            </div>
            <CopyButton
              className={styles.addressItemCopyButton}
              title="Copy address as base64"
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
                    {renderStackElement(el, nestedKeyPrefix, originalIndex)}
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
              {expandedItem === keyPrefix ? element.value : truncateMiddle(element.value, 40)}
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

  const renderNewItemForm = (): JSX.Element => {
    const getTypeClassName = (type: string) => {
      switch (type) {
        case "Integer":
          return styles.integerItem
        case "Cell":
          return styles.cellItem
        case "Slice":
          return styles.sliceItem
        case "Address":
          return styles.addressItem
        case "Null":
          return styles.nullItem
        default:
          return ""
      }
    }

    return (
      <>
        <div className={styles.stackIndex}>{Array.isArray(stack) ? stack.length : 0}</div>
        <div className={styles.stackItemContainer}>
          <div className={`${getTypeClassName(newItemForm.type)} ${styles.newItemFormItem}`}>
            <div className={styles.newItemFormContent}>
              <select
                value={newItemForm.type}
                onChange={e => {
                  setNewItemForm({
                    ...newItemForm,
                    type: e.target.value as StackItemForm["type"],
                    value: "",
                  })
                  setValidationError(null)
                }}
                onKeyDown={e => {
                  if (e.key === "Enter") {
                    e.preventDefault()
                    if (newItemForm.type === "Null") {
                      handleSaveNewItem()
                    } else {
                      // Focus the input field
                      const input = e.currentTarget.parentElement?.querySelector("input")
                      input?.focus()
                    }
                  } else if (e.key === "Escape") {
                    e.preventDefault()
                    handleCancelAddingItem()
                  }
                }}
                className={styles.inlineTypeSelect}
              >
                <option value="Integer">Integer</option>
                <option value="Cell">Cell</option>
                <option value="Slice">Slice</option>
                <option value="Address">Address</option>
                <option value="Null">Null</option>
              </select>

              {newItemForm.type === "Null" ? (
                <div className={styles.nullValuePlaceholder}></div>
              ) : (
                <input
                  type="text"
                  value={newItemForm.value}
                  onChange={e => {
                    const newValue = e.target.value
                    setNewItemForm({...newItemForm, value: newValue})
                    // Clear validation error when user starts typing
                    if (validationError) {
                      setValidationError(null)
                    }
                  }}
                  onKeyDown={e => {
                    if (e.key === "Enter") {
                      e.preventDefault()
                      handleSaveNewItem()
                    } else if (e.key === "Escape") {
                      e.preventDefault()
                      handleCancelAddingItem()
                    }
                  }}
                  placeholder={
                    newItemForm.type === "Integer"
                      ? ""
                      : newItemForm.type === "Cell"
                        ? "BoC hex"
                        : newItemForm.type === "Address"
                          ? "0:abc123... or EQD..."
                          : "Hex data"
                  }
                  className={`${styles.inlineValueInput} ${validationError ? styles.inputError : ""}`}
                  autoFocus
                />
              )}
            </div>
          </div>
        </div>
        <div className={styles.stackItemActions}>
          <Button
            className={styles.stackNavigationButton}
            variant="ghost"
            size="sm"
            onClick={handleSaveNewItem}
            disabled={
              (newItemForm.type !== "Null" && !newItemForm.value.trim()) || !!validationError
            }
            title="Save"
          >
            <FiCheck size={14} />
          </Button>
          <Button
            className={styles.stackNavigationButton}
            variant="ghost"
            size="sm"
            onClick={handleCancelAddingItem}
            title="Cancel"
          >
            <FiX size={14} />
          </Button>
        </div>
      </>
    )
  }

  const itemVariants = {
    initial: {opacity: 0, y: 20},
    animate: {opacity: 1, y: 0},
    exit: {opacity: 0, y: -20},
  }

  const itemsToRender = Array.isArray(stack)
    ? stack.map((el, index) => ({
        element: el,
        key: getElementKey(el, index),
        originalIndex: stack.length - 1 - index,
      }))
    : []

  return (
    <div className={styles.stackEditor}>
      <div className={styles.stackContainer}>
        <div className={styles.stackHeader}>
          <h4>Initial Stack</h4>
          <div className={`${styles.stackHeaderActions} stack-header-actions`}>
            <Button
              className={styles.stackButton}
              variant="ghost"
              size="sm"
              onClick={handleStartAddingItem}
              disabled={isAddingNewItem}
              title="Add new item"
            >
              <FiPlus size={16} />
            </Button>
            <Button
              className={styles.stackButton}
              variant="ghost"
              size="sm"
              onClick={() => setShowTextImport(!showTextImport)}
              title="Import from text"
            >
              <FiFileText size={16} />
              Import
            </Button>
            <Button
              className={styles.stackButton}
              variant="ghost"
              size="sm"
              onClick={clearStack}
              disabled={!Array.isArray(stack) || stack.length === 0}
            >
              <RiFileCloseLine size={16} />
              Clear All
            </Button>
          </div>
        </div>

        {showTextImport && (
          <div className={styles.textImportSection}>
            <textarea
              value={textStackInput}
              onChange={e => setTextStackInput(e.target.value)}
              placeholder="Paste stack text from logs (e.g., [ 42 CS{...} ])"
              className={styles.textImportInput}
              rows={3}
            />
            {parseError && <div className={styles.parseError}>{parseError}</div>}
            <div className={styles.textImportActions}>
              <Button
                className={styles.applyStackButton}
                onClick={parseTextStack}
                disabled={!textStackInput.trim()}
              >
                <FiCheck size={16} />
                Apply
              </Button>
              <Button
                className={styles.stackButton}
                variant="ghost"
                onClick={() => setShowTextImport(false)}
              >
                Cancel
              </Button>
            </div>
          </div>
        )}

        {(!Array.isArray(stack) || stack.length === 0) && !isAddingNewItem ? (
          <div className={styles.emptyStack}>Empty stack</div>
        ) : (
          <div className={styles.stackItems}>
            <AnimatePresence mode="popLayout">
              {isAddingNewItem && (
                <motion.div
                  key="new-item-form"
                  layout
                  variants={itemVariants}
                  initial="initial"
                  animate="animate"
                  exit="exit"
                  transition={{type: "spring", stiffness: 300, damping: 30}}
                  className={styles.stackElement}
                >
                  {renderNewItemForm()}
                </motion.div>
              )}
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
                  <div className={styles.stackItemContainer}>
                    {renderStackElement(element, key, originalIndex)}
                  </div>
                  <div className={styles.stackItemActions}>
                    <Button
                      className={styles.stackNavigationButton}
                      variant="ghost"
                      size="sm"
                      onClick={() => moveStackItem(originalIndex, "up")}
                      disabled={!Array.isArray(stack) || originalIndex === 0}
                      title="Move up"
                    >
                      <FiArrowUp size={14} />
                    </Button>
                    <Button
                      className={styles.stackNavigationButton}
                      variant="ghost"
                      size="sm"
                      onClick={() => moveStackItem(originalIndex, "down")}
                      disabled={!Array.isArray(stack) || originalIndex === stack.length - 1}
                      title="Move down"
                    >
                      <FiArrowDown size={14} />
                    </Button>
                    <Button
                      className={styles.stackNavigationButton}
                      variant="ghost"
                      size="sm"
                      onClick={() => removeStackItem(originalIndex)}
                      title="Remove"
                    >
                      <FiTrash2 size={14} />
                    </Button>
                  </div>
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>
    </div>
  )
}

export default memo(StackEditor)
