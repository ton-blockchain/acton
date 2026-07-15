import {ModeViewer} from "../ModeViewer"
import {parseChangeLibraryMode} from "./parser"

export interface ChangeLibraryModeViewerProps {
  readonly mode: number | undefined
}

export function ChangeLibraryModeViewer({mode}: ChangeLibraryModeViewerProps) {
  return <ModeViewer mode={mode} parseMode={parseChangeLibraryMode} />
}
