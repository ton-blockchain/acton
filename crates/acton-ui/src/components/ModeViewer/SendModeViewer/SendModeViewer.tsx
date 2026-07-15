import {ModeViewer} from "../ModeViewer"
import {parseSendMode} from "./parser"

export interface SendModeViewerProps {
  readonly mode: number | undefined
}

export function SendModeViewer({mode}: SendModeViewerProps) {
  return <ModeViewer mode={mode} parseMode={parseSendMode} />
}
