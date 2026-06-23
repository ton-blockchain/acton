import {useCallback} from "react"
import type {MouseEvent} from "react"
import {type NavigateFunction, useNavigate} from "react-router-dom"

export type ExplorerNavigationClickEvent = MouseEvent<HTMLElement>

export function useOpenExplorerPath(): (
  path: string,
  event?: ExplorerNavigationClickEvent,
) => void {
  const navigate = useNavigate()

  return useCallback(
    (path: string, event?: ExplorerNavigationClickEvent) => {
      openExplorerPath(navigate, path, event)
    },
    [navigate],
  )
}

export function openExplorerPath(
  navigate: NavigateFunction,
  path: string,
  event?: ExplorerNavigationClickEvent,
): void {
  if (event && shouldOpenInNewTab(event)) {
    event.preventDefault()
    globalThis.open(new URL(path, globalThis.location.href).href, "_blank", "noopener,noreferrer")
    return
  }

  void navigate(path)
}

function shouldOpenInNewTab(event: ExplorerNavigationClickEvent): boolean {
  return event.metaKey || event.ctrlKey || event.shiftKey || event.button === 1
}
