import type {ModeInfo} from "../ModeViewer"

export type ChangeLibraryModeInfo = ModeInfo

const CHANGE_LIBRARY_BOUNCE_ON_ERROR = 16
const CHANGE_LIBRARY_MODES_DOCS_URL =
  "https://docs.ton.org/foundations/actions/change-library#modes"

export const CHANGE_LIBRARY_MODE_CONSTANTS = {
  remove: {
    name: "ChangeLibraryModeRemove",
    value: 0,
    description:
      "Removes the library from the contract's library collection. If the library is not present, the action succeeds without changing anything.",
    docsUrl: CHANGE_LIBRARY_MODES_DOCS_URL,
  },
  addPrivate: {
    name: "ChangeLibraryModeAddPrivate",
    value: 1,
    description:
      "Adds the code as a private library available to the current contract. If the library already exists, its public or private status is changed to private.",
    docsUrl: CHANGE_LIBRARY_MODES_DOCS_URL,
  },
  addPublic: {
    name: "ChangeLibraryModeAddPublic",
    value: 2,
    description:
      "Adds the code as a public library. It becomes globally available only when the contract is in the masterchain; in other workchains this behaves like adding a private library.",
    docsUrl: CHANGE_LIBRARY_MODES_DOCS_URL,
  },
  bounceOnError: {
    name: "ChangeLibraryModeBounceOnError",
    value: CHANGE_LIBRARY_BOUNCE_ON_ERROR,
    description: [
      "Initiates the bounce phase if the change-library action fails. Invalid mode bits and combining ",
      {name: "ChangeLibraryModeAddPrivate", value: 1},
      " with ",
      {name: "ChangeLibraryModeAddPublic", value: 2},
      " make the action fail.",
    ],
    docsUrl: CHANGE_LIBRARY_MODES_DOCS_URL,
  },
} as const

export function parseChangeLibraryMode(mode: number): ChangeLibraryModeInfo[] {
  const flags: ChangeLibraryModeInfo[] = []
  const baseMode = mode & 0b11

  if (baseMode === 0) {
    flags.push(CHANGE_LIBRARY_MODE_CONSTANTS.remove)
  } else {
    if (baseMode & CHANGE_LIBRARY_MODE_CONSTANTS.addPrivate.value) {
      flags.push(CHANGE_LIBRARY_MODE_CONSTANTS.addPrivate)
    }
    if (baseMode & CHANGE_LIBRARY_MODE_CONSTANTS.addPublic.value) {
      flags.push(CHANGE_LIBRARY_MODE_CONSTANTS.addPublic)
    }
  }

  if (mode & CHANGE_LIBRARY_BOUNCE_ON_ERROR) {
    flags.push(CHANGE_LIBRARY_MODE_CONSTANTS.bounceOnError)
  }

  const unknownBits = mode & ~(0b11 | CHANGE_LIBRARY_BOUNCE_ON_ERROR)
  if (unknownBits !== 0) {
    flags.push({
      name: "ChangeLibraryModeUnknownBits",
      value: unknownBits,
      description: `Bits 0x${unknownBits.toString(16)} are not defined for change-library actions. Any unsupported mode bits make the action fail.`,
      docsUrl: CHANGE_LIBRARY_MODES_DOCS_URL,
    })
  }

  return flags
}
