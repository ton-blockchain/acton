import {useState, useEffect} from "react"

interface UseTutorialProps {
  readonly tutorialKey: string
  readonly autoStart?: boolean
}

export const useTutorial = ({tutorialKey, autoStart = true}: UseTutorialProps) => {
  const [isOpen, setIsOpen] = useState(false)
  const storageKey = `tutorial-completed-${tutorialKey}`

  useEffect(() => {
    const isCompleted = localStorage.getItem(storageKey) === "true"
    if (autoStart && !isCompleted) {
      setIsOpen(true)
    }
  }, [storageKey, autoStart])

  const startTutorial = () => setIsOpen(true)

  const closeTutorial = () => {
    // don't show tutorial anymore on explicit closing
    localStorage.setItem(storageKey, "true")
    setIsOpen(false)
  }

  const completeTutorial = () => {
    localStorage.setItem(storageKey, "true")
    setIsOpen(false)
  }

  const resetTutorial = () => {
    localStorage.removeItem(storageKey)
  }

  return {
    isOpen,
    startTutorial,
    closeTutorial,
    completeTutorial,
    resetTutorial,
  }
}
