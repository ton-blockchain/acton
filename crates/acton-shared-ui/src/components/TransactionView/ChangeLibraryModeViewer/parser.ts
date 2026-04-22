export interface ChangeLibraryModeInfo {
  readonly name: string
  readonly value: number
  readonly description: string
}

const CHANGE_LIBRARY_BOUNCE_ON_ERROR = 16

export const CHANGE_LIBRARY_MODE_CONSTANTS = {
  remove: {
    name: "ChangeLibraryModeRemove",
    value: 0,
    description: "Remove the library from the account.",
  },
  addPrivate: {
    name: "ChangeLibraryModeAddPrivate",
    value: 1,
    description: "Add the library as a private library.",
  },
  addPublic: {
    name: "ChangeLibraryModeAddPublic",
    value: 2,
    description: "Add the library as a public library.",
  },
  bounceOnError: {
    name: "ChangeLibraryModeBounceOnError",
    value: CHANGE_LIBRARY_BOUNCE_ON_ERROR,
    description: "Bounce the transaction if the library action fails.",
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
      description: `Unknown change library mode bits: 0x${unknownBits.toString(16)}.`,
    })
  }

  return flags
}
