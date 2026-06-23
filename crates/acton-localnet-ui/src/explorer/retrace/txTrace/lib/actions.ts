import type {OutAction} from "@ton/core"

import {formatCurrency, formatAddress} from "@retrace/lib/format"

export const getActionSummary = (action: OutAction) => {
  switch (action.type) {
    case "sendMsg": {
      const msg = action.outMsg
      const msgType = msg.info.type === "internal" ? "Internal" : "External"
      const rawDest =
        msg.info.type === "internal"
          ? msg.info.dest
          : msg.info.type === "external-out"
            ? msg.info.dest
            : "unknown"
      const dest = typeof rawDest === "string" ? rawDest : formatAddress(rawDest)
      const value = msg.info.type === "internal" ? formatCurrency(msg.info.value.coins) : ""
      return {
        title: "Send Message",
        icon: "send-icon",
        description: `${msgType} → ${dest}`,
        value: value,
      }
    }
    case "setCode":
      return {
        title: "Set Code",
        icon: "code-icon",
        description: "Update contract code",
        value: "",
      }
    case "reserve":
      return {
        title: "Reserve",
        icon: "reserve-icon",
        description: `Mode: ${action.mode}`,
        value: formatCurrency(action.currency.coins),
      }
    default:
      return {
        title: "Unknown",
        icon: "unknown-icon",
        description: `Type: ${action.type}`,
        value: "",
      }
  }
}
