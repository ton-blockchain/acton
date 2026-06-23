import React, {useState, useEffect} from "react"

import inlineLoaderStyles from "@retrace/ui/InlineLoader/InlineLoader.module.css"

import styles from "./FullScreenLoader.module.css"

const MESSAGES = [
  "Welcome to TxTracer! Loading core components...",
  "Preparing local sandbox...",
  "Initializing editor...",
  "Loading core components...",
  "Fetching initial data...",
  "Finalizing setup...",
  "Almost there!",
]

interface FullScreenLoaderProps {
  readonly baseMessage?: string
  readonly subtext?: string
  readonly loading?: boolean
}

const FullScreenLoader: React.FC<FullScreenLoaderProps> = ({
  baseMessage,
  subtext,
  loading = true,
}) => {
  const [currentMessageIndex, setCurrentMessageIndex] = useState(0)

  useEffect(() => {
    if (!baseMessage) {
      const intervalId = setInterval(() => {
        setCurrentMessageIndex(prevIndex => (prevIndex + 1) % MESSAGES.length)
      }, 3000)

      return () => clearInterval(intervalId)
    }
  }, [baseMessage])

  const displayMessage = baseMessage || MESSAGES[currentMessageIndex]

  return (
    <div
      className={`${styles.fullScreenContainer} ${loading ? "" : styles.hidden}`}
      role="status"
      aria-live="polite"
      aria-hidden={!loading}
    >
      <div className={styles.contentWrapper}>
        <div className={inlineLoaderStyles.spinnerWrapper}>
          <div className={inlineLoaderStyles.spinner} />
          <div className={inlineLoaderStyles.spinnerGlow} />
        </div>
        {displayMessage && <div className={inlineLoaderStyles.loadingText}>{displayMessage}</div>}
        {subtext && (
          <div className={inlineLoaderStyles.loadingSubtext}>
            <span>{subtext}</span>
            <span className={inlineLoaderStyles.dotAnimation}>
              <span className={inlineLoaderStyles.dot}>.</span>
              <span className={inlineLoaderStyles.dot}>.</span>
              <span className={inlineLoaderStyles.dot}>.</span>
            </span>
          </div>
        )}
      </div>
    </div>
  )
}

export default FullScreenLoader
