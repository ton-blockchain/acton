import {SendModeInfo} from "@/components/TransactionView/SendModeViewer/SendModeViewer";

export const SEND_MODE_CONSTANTS = {
  0: {name: "SendDefaultMode", description: "Ordinary message (default)."},
  64: {
    name: "SendRemainingValue",
    description: "Carry all the remaining value of the inbound message.",
  },
  128: {
    name: "SendRemainingBalance",
    description: "Carry all the remaining balance of the current smart contract.",
  },
  1: {
    name: "SendPayFwdFeesSeparately",
    description: "Pay forward fees separately from the message value.",
  },
  2: {
    name: "SendIgnoreErrors",
    description: "Ignore any errors arising while processing this message.",
  },
  16: {
    name: "SendBounceIfActionFail",
    description: "Bounce transaction in case of any errors during action phase.",
  },
  32: {
    name: "SendDestroyIfZero",
    description: "Current account will be destroyed if its resulting balance is zero.",
  },
} as const

export function parseSendMode(mode: number): SendModeInfo[] {
  const flags: SendModeInfo[] = []
  for (const [value, constant] of Object.entries(SEND_MODE_CONSTANTS)) {
    const flagValue = Number.parseInt(value, 10)
    if (flagValue === 0) continue
    if (mode & flagValue) {
      flags.push({name: constant.name, value: flagValue, description: constant.description})
    }
  }
  if (flags.length === 0 && mode === 0) {
    flags.push({
      name: SEND_MODE_CONSTANTS[0].name,
      value: 0,
      description: SEND_MODE_CONSTANTS[0].description,
    })
  }
  return flags
}
