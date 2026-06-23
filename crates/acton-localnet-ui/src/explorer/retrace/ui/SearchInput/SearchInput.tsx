import React, {useState} from "react"

import Button from "@retrace/ui/Button"

import styles from "./SearchInput.module.css"

interface SearchInputProps {
  readonly value: string
  readonly onChange: (v: string) => void
  readonly onSubmit: () => void
  readonly onFocus?: () => void
  readonly onBlur?: () => void
  readonly placeholder?: string
  readonly loading?: boolean
  readonly autoFocus?: boolean
  readonly compact?: boolean
  readonly onInputClick?: (event: React.MouseEvent<HTMLInputElement>) => void
  readonly buttonLabel?: string
}

const SearchInput: React.FC<SearchInputProps> = ({
  value,
  onChange,
  onSubmit,
  onFocus,
  onBlur,
  placeholder,
  loading,
  autoFocus = false,
  compact = false,
  onInputClick,
  buttonLabel = "Trace",
}) => {
  const [focused, setFocused] = useState(false)

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") onSubmit()
  }

  const wrapperClass = `${styles.inputWrapper} ${focused ? styles.focused : ""} ${compact ? styles.compactWrapper : ""}`
  const inputClass = `${styles.txInput} ${compact ? styles.compactInput : ""}`
  const buttonClass = `${styles.submitButton} ${compact ? styles.compactButton : ""}`

  return (
    <div className={wrapperClass} role="search">
      <div
        className={`${styles.searchIcon} ${compact ? styles.compactSearchIcon : ""}`}
        aria-hidden="true"
      >
        <svg
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          aria-hidden="true"
        >
          <path
            d="M11 19C15.4183 19 19 15.4183 19 11C19 6.58172 15.4183 3 11 3C6.58172 3 3 6.58172 3 11C3 15.4183 6.58172 19 11 19Z"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M21 21L16.65 16.65"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </div>
      <input
        type="text"
        spellCheck="false"
        autoFocus={autoFocus}
        placeholder={placeholder}
        value={value}
        onChange={e => onChange(e.target.value)}
        onKeyDown={handleKeyPress}
        onFocus={() => {
          onFocus?.()
          setFocused(true)
        }}
        onBlur={() => {
          onBlur?.()
          setFocused(false)
        }}
        onClick={onInputClick}
        className={inputClass}
        aria-label={placeholder ?? "Search transaction"}
        aria-describedby={compact ? undefined : "search-instructions"}
      />
      <Button
        variant="primary"
        size={compact ? "sm" : "md"}
        className={buttonClass}
        onClick={onSubmit}
        disabled={loading}
        aria-label={loading ? `${buttonLabel}...` : buttonLabel}
        title={loading ? `${buttonLabel}...` : `${buttonLabel} (Enter)`}
      >
        {buttonLabel}
      </Button>
      {!compact && (
        <div id="search-instructions" className="sr-only">
          Press Enter or click Trace button to search. Use Ctrl+Enter or Cmd+Enter as shortcut.
        </div>
      )}
    </div>
  )
}

export default SearchInput
