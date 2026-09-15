interface ExitCodeDescription {
  readonly name: string
  readonly description: string
  readonly phase: string
  readonly docsAnchor?: string
}

/** Metadata for a standard TVM exit code known to the base UI library. */
export interface StandardExitCodeInfo {
  readonly name: string
  readonly description: string
  readonly phase: string
  readonly docsUrl?: string
}

export type ExitCodePhase = "compute" | "action"

/**
 * Minimal structural projection of a contract ABI used to resolve contract-defined exit codes.
 *
 * Full compiler ABI objects are assignable to this type, but ExitCodeChip only reads the thrown
 * error code, symbolic name, and optional description. Keeping this projection local prevents
 * @acton/ui from depending on an ABI compiler package or pulling its complete type graph.
 */
export interface ExitCodeAbi {
  readonly thrown_errors?: readonly {
    readonly err_code: number
    readonly name?: string
    readonly description?: string
  }[]
}

export interface ExitCodeInfo {
  readonly customSymbolicName?: string
  readonly description: string
  readonly displayName: string
  readonly docsUrl?: string
  readonly isSuccess: boolean
  readonly origin: string
}

interface CustomExitCodeInfo {
  readonly symbolicName: string
  readonly description: string
}

const EXIT_CODE_DESCRIPTIONS: Readonly<Record<number, ExitCodeDescription>> = {
  0: {
    docsAnchor: "#0-normal-termination",
    name: "Success",
    description: "Standard successful execution exit code.",
    phase: "Compute and action phases",
  },
  1: {
    docsAnchor: "#1-alternative-termination",
    name: "Alternative success",
    description: "Alternative successful execution exit code. Reserved, but does not occur.",
    phase: "Compute phase",
  },
  2: {
    docsAnchor: "#2-stack-underflow",
    name: "Stack underflow",
    description: "Stack underflow.",
    phase: "Compute phase",
  },
  3: {
    docsAnchor: "#3-stack-overflow",
    name: "Stack overflow",
    description: "Stack overflow.",
    phase: "Compute phase",
  },
  4: {
    docsAnchor: "#4-integer-overflow",
    name: "Integer overflow",
    description: "Integer overflow, or division/modulo by zero.",
    phase: "Compute phase",
  },
  5: {
    docsAnchor: "#5-integer-out-of-expected-range",
    name: "Range check error",
    description: "Range check error — an integer is out of its expected range.",
    phase: "Compute phase",
  },
  6: {
    docsAnchor: "#6-invalid-opcode",
    name: "Invalid opcode",
    description: "Instruction or its Fift mapping not found.",
    phase: "Compute phase",
  },
  7: {
    docsAnchor: "#7-type-check-error",
    name: "Type check error",
    description: "Type check error.",
    phase: "Compute phase",
  },
  8: {
    docsAnchor: "#8-cell-overflow",
    name: "Cell overflow",
    description: "Cell overflow.",
    phase: "Compute phase",
  },
  9: {
    docsAnchor: "#9-cell-underflow",
    name: "Cell underflow",
    description: "Cell underflow.",
    phase: "Compute phase",
  },
  10: {
    docsAnchor: "#10-dictionary-error",
    name: "Dictionary error",
    description: "Dictionary error.",
    phase: "Compute phase",
  },
  11: {
    docsAnchor: '#11-"unknown"-error',
    name: "Unknown error",
    description: "Unknown error, may be thrown by user programs.",
    phase: "Compute phase",
  },
  12: {
    docsAnchor: "#12-fatal-error",
    name: "Fatal error",
    description: "Fatal error. Thrown by TVM in situations deemed impossible.",
    phase: "Compute phase",
  },
  13: {
    docsAnchor: "#13-out-of-gas-error",
    name: "Out of Gas",
    description: "Not enough gas to finish execution (shown as -14 at runtime).",
    phase: "Compute phase",
  },
  [-14]: {
    docsAnchor: "#14-out-of-gas-error",
    name: "Out of Gas (Negative)",
    description: "Same as 13. Negative, so that it cannot be faked.",
    phase: "Compute phase",
  },
  14: {
    docsAnchor: "#14-virtualization-error",
    name: "Virtualization error",
    description: "Virtualization error. Reserved, but never thrown.",
    phase: "Compute phase",
  },
  32: {
    docsAnchor: "#32-action-list-is-invalid",
    name: "Action list invalid",
    description: "Action list is invalid.",
    phase: "Action phase",
  },
  33: {
    docsAnchor: "#33-action-list-is-too-long",
    name: "Action list too long",
    description: "Action list is too long.",
    phase: "Action phase",
  },
  34: {
    docsAnchor: "#34-invalid-or-unsupported-action",
    name: "Action invalid",
    description: "Action is invalid or not supported.",
    phase: "Action phase",
  },
  35: {
    docsAnchor: "#35-invalid-source-address-in-outbound-message",
    name: "Invalid source address",
    description: "Invalid source address in outbound message.",
    phase: "Action phase",
  },
  36: {
    docsAnchor: "#36-invalid-destination-address-in-outbound-message",
    name: "Invalid destination address",
    description: "Invalid destination address in outbound message.",
    phase: "Action phase",
  },
  37: {
    docsAnchor: "#37-not-enough-grams",
    name: "Not enough GRAMs",
    description: "Not enough GRAMs.",
    phase: "Action phase",
  },
  38: {
    docsAnchor: "#38-not-enough-extra-currencies",
    name: "Not enough extra currencies",
    description: "Not enough extra currencies.",
    phase: "Action phase",
  },
  39: {
    docsAnchor: "#39-outbound-message-does-not-fit-into-cell",
    name: "Message does not fit",
    description: "Outbound message does not fit into a cell after rewriting.",
    phase: "Action phase",
  },
  40: {
    docsAnchor: "#40-cannot-process-message",
    name: "Cannot process message",
    description:
      "Cannot process a message — not enough funds, the message is too large, or its Merkle depth is too big.",
    phase: "Action phase",
  },
  41: {
    docsAnchor: "#41-library-reference-is-null",
    name: "Library reference null",
    description: "Library reference is null during library change action.",
    phase: "Action phase",
  },
  42: {
    docsAnchor: "#42-library-change-action-error",
    name: "Library change action error",
    description: "Library change action error.",
    phase: "Action phase",
  },
  43: {
    docsAnchor: "#43-library-limits-exceeded",
    name: "Library limits exceeded",
    description:
      "Exceeded the maximum number of cells in the library or the maximum depth of the Merkle tree.",
    phase: "Action phase",
  },
  50: {
    docsAnchor: "#50-account-state-size-exceeded-limits",
    name: "Account state size exceeded",
    description: "Account state size exceeded limits.",
    phase: "Action phase",
  },
  63: {
    name: "Opcode does not match",
    description: "Default opcode mismatch for `T.fromCell` and `T.fromSlice` in Tolk.",
    phase: "Compute phase",
  },
  65_535: {
    name: "Unknown opcode",
    description: "Common developer-defined code often used similarly to 130 (unknown opcode).",
    phase: "User-defined",
  },
}

const TVM_EXIT_CODES_DOCS_BASE_URL = "https://docs.ton.org/tvm/exit-codes"
const UNKNOWN_EXIT_CODE_DESCRIPTION =
  "Contract returned a user-defined exit code that is not declared in the ABI, so no symbolic description is available for this value."

const getExitCodeDocsUrl = (exitCode: number): string | undefined => {
  const description = EXIT_CODE_DESCRIPTIONS[exitCode]
  return description?.docsAnchor
    ? `${TVM_EXIT_CODES_DOCS_BASE_URL}${description.docsAnchor}`
    : undefined
}

/** Returns standard TVM metadata, or undefined for contract-defined and unknown exit codes. */
export function getStandardExitCodeInfo(exitCode: number): StandardExitCodeInfo | undefined {
  const standardDescription = EXIT_CODE_DESCRIPTIONS[exitCode]
  if (!standardDescription) {
    return undefined
  }

  return {
    name: standardDescription.name,
    description: standardDescription.description,
    phase: standardDescription.phase,
    docsUrl: getExitCodeDocsUrl(exitCode),
  }
}

const getCustomExitCodeInfo = (
  exitCode: number,
  abi: ExitCodeAbi | undefined,
): CustomExitCodeInfo | undefined => {
  const thrownError = abi?.thrown_errors?.find(error => error.err_code === exitCode)
  const symbolicName = thrownError?.name

  if (!symbolicName) return undefined

  return {
    symbolicName,
    description: thrownError.description ?? symbolicName,
  }
}

const getPhaseLabel = (phase: ExitCodePhase): string =>
  phase === "action" ? "Action phase" : "Compute phase"

export function resolveExitCode(
  exitCode: number,
  abi: ExitCodeAbi | undefined,
  phase: ExitCodePhase,
): ExitCodeInfo {
  const standardExitCode = getStandardExitCodeInfo(exitCode)
  const customExitCode = getCustomExitCodeInfo(exitCode, abi)

  return {
    customSymbolicName: customExitCode?.symbolicName,
    displayName: standardExitCode?.name ?? customExitCode?.symbolicName ?? "Custom exit code",
    description:
      standardExitCode?.description ?? customExitCode?.description ?? UNKNOWN_EXIT_CODE_DESCRIPTION,
    origin: standardExitCode?.phase ?? getPhaseLabel(phase),
    docsUrl: standardExitCode?.docsUrl,
    isSuccess: phase === "action" ? exitCode === 0 : exitCode === 0 || exitCode === 1,
  }
}
