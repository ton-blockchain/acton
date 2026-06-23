import React, {useCallback, useEffect, useRef, useState} from "react"
import {FiSettings} from "react-icons/fi"

import styles from "./SettingsDropdown.module.css"

export type SettingsItem = {
  readonly id: string
  readonly label: string
  readonly checked: boolean
  readonly onToggle: () => void
}

interface SettingsDropdownProps {
  readonly items: readonly SettingsItem[]
}

const SettingsDropdown: React.FC<SettingsDropdownProps> = ({items}) => {
  const [isOpen, setIsOpen] = useState(false)
  const settingsRef = useRef<HTMLDivElement | null>(null)
  const settingsButtonRef = useRef<HTMLButtonElement | null>(null)

  const handleSettingsKeyDown = useCallback((event: React.KeyboardEvent) => {
    switch (event.key) {
      case "Escape": {
        setIsOpen(false)
        settingsButtonRef.current?.focus()
        break
      }
      case "ArrowDown": {
        event.preventDefault()
        const firstCheckbox: HTMLElement | null | undefined =
          settingsRef.current?.querySelector('input[type="checkbox"]')
        firstCheckbox?.focus()
        break
      }
    }
  }, [])

  const handleItemKeyDown = useCallback((event: React.KeyboardEvent, action: () => void) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault()
      action()
    }
    if (event.key === "Escape") {
      setIsOpen(false)
      settingsButtonRef.current?.focus()
    }
  }, [])

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (settingsRef.current && !settingsRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isOpen) {
        setIsOpen(false)
        settingsButtonRef.current?.focus()
      }
    }
    document.addEventListener("mousedown", handleClickOutside)
    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("mousedown", handleClickOutside)
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [isOpen])

  return (
    <div className={styles.settingsContainer} ref={settingsRef}>
      <button
        type="button"
        className={styles.settingsButton}
        title="Settings"
        onClick={() => setIsOpen(prev => !prev)}
        ref={settingsButtonRef}
        onKeyDown={handleSettingsKeyDown}
        aria-label="Open settings menu"
        aria-expanded={isOpen}
        aria-haspopup="menu"
        aria-controls="settings-menu"
        data-testid="settings-button"
      >
        <FiSettings size={16} aria-hidden="true" />
      </button>
      {isOpen && (
        <div
          className={styles.settingsDropdown}
          role="menu"
          id="settings-menu"
          aria-label="Settings menu"
        >
          {items.map(item => (
            <label key={item.id} className={styles.settingsItem} aria-checked={item.checked}>
              <input
                type="checkbox"
                checked={item.checked}
                onChange={item.onToggle}
                onKeyDown={event => handleItemKeyDown(event, item.onToggle)}
                aria-describedby={`${item.id}-desc`}
                tabIndex={0}
              />
              <span className={styles.checkboxCustom} aria-hidden="true"></span>
              <span className={styles.checkboxLabel} id={`${item.id}-desc`}>
                {item.label}
              </span>
            </label>
          ))}
        </div>
      )}
    </div>
  )
}

export default SettingsDropdown
