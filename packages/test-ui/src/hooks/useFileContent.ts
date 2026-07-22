import {useEffect, useState} from "react"

import {getErrorMessage, isAbortError} from "./request"

interface FileContentState {
  readonly filePath: string | undefined
  readonly content: string | undefined
  readonly error: string | undefined
  readonly loading: boolean
}

const EMPTY_FILE_CONTENT: FileContentState = {
  filePath: undefined,
  content: undefined,
  error: undefined,
  loading: false,
}

export function useFileContent(filePath: string | undefined) {
  const [state, setState] = useState<FileContentState>(EMPTY_FILE_CONTENT)

  useEffect(() => {
    if (filePath === undefined) {
      setState(EMPTY_FILE_CONTENT)
      return
    }

    const controller = new AbortController()
    setState({filePath, content: undefined, error: undefined, loading: true})

    const loadContent = async () => {
      try {
        const response = await fetch(`/api/file?path=${encodeURIComponent(filePath)}`, {
          signal: controller.signal,
        })
        if (!response.ok) {
          throw new Error(`Failed to fetch file content: ${response.status}`)
        }

        const content = await response.text()
        setState({filePath, content, error: undefined, loading: false})
      } catch (error) {
        if (isAbortError(error)) return

        console.error("Failed to fetch file content", {filePath, error})
        setState({filePath, content: undefined, error: getErrorMessage(error), loading: false})
      }
    }

    void loadContent()
    return () => controller.abort()
  }, [filePath])

  if (filePath === undefined) return EMPTY_FILE_CONTENT
  if (state.filePath === filePath) return state

  return {filePath, content: undefined, error: undefined, loading: true}
}
