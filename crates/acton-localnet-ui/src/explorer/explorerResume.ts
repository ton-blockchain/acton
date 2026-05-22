export const EXPLORER_HISTORY_STORAGE_KEY = "explorer_history"

const EXPLORER_INPUT_STORAGE_KEY = "explorer_input"
const EXPLORER_LAST_PATH_STORAGE_KEY = "explorer_last_path"

export function readExplorerInput(): string {
  return localStorage.getItem(EXPLORER_INPUT_STORAGE_KEY) ?? ""
}

export function writeExplorerInput(input: string): void {
  localStorage.setItem(EXPLORER_INPUT_STORAGE_KEY, input)
}

export function readExplorerLastPath(): string {
  const path = localStorage.getItem(EXPLORER_LAST_PATH_STORAGE_KEY)
  return path && isExplorerPath(path) ? path : "/explorer"
}

export function writeExplorerLastPath(path: string): void {
  if (isExplorerPath(path)) {
    localStorage.setItem(EXPLORER_LAST_PATH_STORAGE_KEY, path)
  }
}

function isExplorerPath(path: string): boolean {
  return path === "/explorer" || path.startsWith("/explorer/")
}
