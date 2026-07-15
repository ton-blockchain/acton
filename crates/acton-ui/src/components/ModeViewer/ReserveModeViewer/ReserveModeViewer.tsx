import {ModeViewer} from "../ModeViewer"
import {parseReserveMode} from "./parser"

export interface ReserveModeViewerProps {
  readonly mode: number | undefined
}

export function ReserveModeViewer({mode}: ReserveModeViewerProps) {
  return <ModeViewer mode={mode} parseMode={parseReserveMode} />
}
