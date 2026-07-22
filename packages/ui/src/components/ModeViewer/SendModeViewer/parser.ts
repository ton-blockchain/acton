import type {ModeInfo} from "../ModeViewer"

export type SendModeInfo = ModeInfo

const SEND_MODES_DOCS_URL = "https://docs.ton.org/foundations/messages/modes"

export const SEND_MODE_CONSTANTS = {
  0: {
    name: "SendModeRegular",
    description:
      "Default sending mode. Forward fees are deducted from the indicated message value. A sending failure rolls back the transaction without producing a bounce message.",
    docsUrl: SEND_MODES_DOCS_URL,
  },
  1: {
    name: "SendModePayFeesSeparately",
    description: [
      "Pays forward fees from the contract balance instead of subtracting them from the outgoing message value. This flag is removed when combined with ",
      {name: "SendModeCarryAllBalance", value: 128},
      ".",
    ],
    docsUrl: SEND_MODES_DOCS_URL,
  },
  2: {
    name: "SendModeIgnoreErrors",
    description:
      "Ignores most message-sending errors and continues the action phase without a bounce. Invalid mode combinations, invalid source addresses, and message-repacking errors are still not ignored.",
    docsUrl: SEND_MODES_DOCS_URL,
  },
  16: {
    name: "SendModeBounceOnActionFail",
    description: [
      "Initiates the bounce phase if the action phase fails. It has no effect when ",
      {name: "SendModeIgnoreErrors", value: 2},
      " is also enabled.",
    ],
    docsUrl: SEND_MODES_DOCS_URL,
  },
  32: {
    name: "SendModeDestroy",
    description: [
      "Destroys the current account only when its resulting balance is zero. This flag has effect only together with ",
      {name: "SendModeCarryAllBalance", value: 128},
      ".",
    ],
    docsUrl: SEND_MODES_DOCS_URL,
  },
  64: {
    name: "SendModeCarryAllRemainingMessageValue",
    description: [
      "Adds the remaining value of the inbound message to the value specified for the outgoing message. Without ",
      {name: "SendModePayFeesSeparately", value: 1},
      ", gas fees and accumulated action fines are deducted from the carried amount.",
    ],
    docsUrl: SEND_MODES_DOCS_URL,
  },
  128: {
    name: "SendModeCarryAllBalance",
    description: [
      "Replaces the specified message value with the contract's entire remaining balance. Use with caution: it can transfer the full balance, and ",
      {name: "SendModePayFeesSeparately", value: 1},
      " is removed.",
    ],
    docsUrl: SEND_MODES_DOCS_URL,
  },
  1024: {
    name: "SendModeEstimateFeeOnly",
    description: [
      "Used by ",
      {
        code: "OutMessage.estimateFeeWithoutSending",
        href: "https://ton-blockchain.github.io/acton/docs/tolk_standard_library/common#outmessageestimatefeewithoutsending",
      },
      " to estimate the forward fees for a prepared message and return them as ",
      {code: "coins"},
      " without adding an output action. The Tolk helper adds this flag automatically.",
    ],
    docsUrl: SEND_MODES_DOCS_URL,
  },
} as const

export function parseSendMode(mode: number): SendModeInfo[] {
  const flags: SendModeInfo[] = []

  for (const [value, constant] of Object.entries(SEND_MODE_CONSTANTS)) {
    const flagValue = Number.parseInt(value, 10)
    if (flagValue !== 0 && mode & flagValue) {
      flags.push({
        name: constant.name,
        value: flagValue,
        description: constant.description,
        docsUrl: constant.docsUrl,
      })
    }
  }

  if (flags.length === 0 && mode === 0) {
    flags.push({
      name: SEND_MODE_CONSTANTS[0].name,
      value: 0,
      description: SEND_MODE_CONSTANTS[0].description,
      docsUrl: SEND_MODE_CONSTANTS[0].docsUrl,
    })
  }

  return flags
}
