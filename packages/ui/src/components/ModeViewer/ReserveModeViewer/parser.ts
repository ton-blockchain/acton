import type {ModeInfo} from "../ModeViewer"

export type ReserveModeInfo = ModeInfo

const RESERVE_MODES_DOCS_URL = "https://docs.ton.org/foundations/actions/reserve#modes"

export const RESERVE_MODE_CONSTANTS = {
  0: {
    name: "ReserveExact",
    description:
      "Reserves exactly the calculated amount of nanograms, leaving only the rest available to subsequent actions. The action fails when the remaining balance is insufficient.",
    docsUrl: RESERVE_MODES_DOCS_URL,
  },
  1: {
    name: "ReserveAllExcept",
    description:
      "Reserves the remaining account balance minus the calculated amount. In the basic case, this leaves at most the specified amount available to subsequent actions.",
    docsUrl: RESERVE_MODES_DOCS_URL,
  },
  2: {
    name: "ReserveAtMost",
    description:
      "Reserves the smaller of the calculated amount and the remaining account balance, avoiding an insufficient-balance failure when the requested reserve is too large.",
    docsUrl: RESERVE_MODES_DOCS_URL,
  },
  4: {
    name: "ReserveAddOriginalBalance",
    description: [
      "Adds the account's original balance before the compute phase, excluding the incoming message value, to the requested amount. With ",
      {name: "ReserveInvertSign", value: 8},
      ", it instead calculates original balance minus amount.",
    ],
    docsUrl: RESERVE_MODES_DOCS_URL,
  },
  8: {
    name: "ReserveInvertSign",
    description: [
      "Negates the requested amount before applying the original-balance adjustment. This flag is valid only together with ",
      {name: "ReserveAddOriginalBalance", value: 4},
      "; otherwise the reserve action is rejected as invalid or unsupported during the action phase (result code 34).",
    ],
    docsUrl: RESERVE_MODES_DOCS_URL,
  },
  16: {
    name: "ReserveBounceIfActionFail",
    description:
      "Initiates the bounce phase if the reservation action fails, for example because the mode is invalid or the contract lacks enough balance.",
    docsUrl: RESERVE_MODES_DOCS_URL,
  },
} as const

export function parseReserveMode(mode: number): ReserveModeInfo[] {
  const flags: ReserveModeInfo[] = []
  const baseMode = mode & 3

  if (baseMode === 0 || baseMode === 1 || baseMode === 2) {
    const constant = RESERVE_MODE_CONSTANTS[baseMode]
    flags.push({
      name: constant.name,
      value: baseMode,
      description: constant.description,
      docsUrl: constant.docsUrl,
    })
  }

  for (const [value, constant] of Object.entries(RESERVE_MODE_CONSTANTS)) {
    const flagValue = Number.parseInt(value, 10)
    if (flagValue >= 4 && mode & flagValue) {
      flags.push({
        name: constant.name,
        value: flagValue,
        description: constant.description,
        docsUrl: constant.docsUrl,
      })
    }
  }

  return flags
}
