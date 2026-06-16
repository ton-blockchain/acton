export interface ExitCodeDescription {
  readonly name: string
  readonly description: string
  readonly phase: string
  readonly docsAnchor?: string
}

export const EXIT_CODE_DESCRIPTIONS = {
  0: {
    docsAnchor: "#0-normal-termination",
    name: "Success",
    description: "Standard successful execution exit code.",
    phase: "Compute and action phases",
  },
  1: {
    docsAnchor: "#1-alternative-termination",
    name: "Alternative Success",
    description: "Alternative successful execution exit code. Reserved, but does not occur.",
    phase: "Compute phase",
  },
  2: {
    docsAnchor: "#2-stack-underflow",
    name: "Stack Underflow",
    description: "Stack underflow.",
    phase: "Compute phase",
  },
  3: {
    docsAnchor: "#3-stack-overflow",
    name: "Stack Overflow",
    description: "Stack overflow.",
    phase: "Compute phase",
  },
  4: {
    docsAnchor: "#4-integer-overflow",
    name: "Integer Overflow",
    description: "Integer overflow, or division/modulo by zero.",
    phase: "Compute phase",
  },
  5: {
    docsAnchor: "#5-integer-out-of-expected-range",
    name: "Range Check Error",
    description: "Range check error — an integer is out of its expected range.",
    phase: "Compute phase",
  },
  6: {
    docsAnchor: "#6-invalid-opcode",
    name: "Invalid Opcode",
    description: "Instruction or its Fift mapping not found.",
    phase: "Compute phase",
  },
  7: {
    docsAnchor: "#7-type-check-error",
    name: "Type Check Error",
    description: "Type check error.",
    phase: "Compute phase",
  },
  8: {
    docsAnchor: "#8-cell-overflow",
    name: "Cell Overflow",
    description: "Cell overflow.",
    phase: "Compute phase",
  },
  9: {
    docsAnchor: "#9-cell-underflow",
    name: "Cell Underflow",
    description: "Cell underflow.",
    phase: "Compute phase",
  },
  10: {
    docsAnchor: "#10-dictionary-error",
    name: "Dictionary Error",
    description: "Dictionary error.",
    phase: "Compute phase",
  },
  11: {
    docsAnchor: '#11-"unknown"-error',
    name: "Unknown Error",
    description: "Unknown error, may be thrown by user programs.",
    phase: "Compute phase",
  },
  12: {
    docsAnchor: "#12-fatal-error",
    name: "Fatal Error",
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
    name: "Virtualization Error",
    description: "Virtualization error. Reserved, but never thrown.",
    phase: "Compute phase",
  },
  32: {
    docsAnchor: "#32-action-list-is-invalid",
    name: "Action List Invalid",
    description: "Action list is invalid.",
    phase: "Action phase",
  },
  33: {
    docsAnchor: "#33-action-list-is-too-long",
    name: "Action List Too Long",
    description: "Action list is too long.",
    phase: "Action phase",
  },
  34: {
    docsAnchor: "#34-invalid-or-unsupported-action",
    name: "Action Invalid",
    description: "Action is invalid or not supported.",
    phase: "Action phase",
  },
  35: {
    docsAnchor: "#35-invalid-source-address-in-outbound-message",
    name: "Invalid Source Address",
    description: "Invalid source address in outbound message.",
    phase: "Action phase",
  },
  36: {
    docsAnchor: "#36-invalid-destination-address-in-outbound-message",
    name: "Invalid Destination Address",
    description: "Invalid destination address in outbound message.",
    phase: "Action phase",
  },
  37: {
    docsAnchor: "#37-not-enough-grams",
    name: "Not Enough GRAMs",
    description: "Not enough GRAMs.",
    phase: "Action phase",
  },
  38: {
    docsAnchor: "#38-not-enough-extra-currencies",
    name: "Not Enough Extra Currencies",
    description: "Not enough extra currencies.",
    phase: "Action phase",
  },
  39: {
    docsAnchor: "#39-outbound-message-does-not-fit-into-cell",
    name: "Message Does Not Fit",
    description: "Outbound message does not fit into a cell after rewriting.",
    phase: "Action phase",
  },
  40: {
    docsAnchor: "#40-cannot-process-message",
    name: "Cannot Process Message",
    description:
      "Cannot process a message — not enough funds, the message is too large, or its Merkle depth is too big.",
    phase: "Action phase",
  },
  41: {
    docsAnchor: "#41-library-reference-is-null",
    name: "Library Reference Null",
    description: "Library reference is null during library change action.",
    phase: "Action phase",
  },
  42: {
    docsAnchor: "#42-library-change-action-error",
    name: "Library Change Action Error",
    description: "Library change action error.",
    phase: "Action phase",
  },
  43: {
    docsAnchor: "#43-library-limits-exceeded",
    name: "Library Limits Exceeded",
    description:
      "Exceeded the maximum number of cells in the library or the maximum depth of the Merkle tree.",
    phase: "Action phase",
  },
  50: {
    docsAnchor: "#50-account-state-size-exceeded-limits",
    name: "Account State Size Exceeded",
    description: "Account state size exceeded limits.",
    phase: "Action phase",
  },
  63: {
    name: "Opcode Does Not Match",
    description: "Default opcode mismatch for `T.fromCell` and `T.fromSlice` in Tolk.",
    phase: "Compute phase",
  },
  65535: {
    name: "Unknown Opcode",
    description: "Common developer-defined code often used similarly to 130 (unknown opcode).",
    phase: "User-defined",
  },
} as const satisfies Record<number, ExitCodeDescription>

const TVM_EXIT_CODES_DOCS_BASE_URL = "https://docs.ton.org/tvm/exit-codes"

export function getExitCodeDocsUrl(exitCode: number): string | undefined {
  const description = (EXIT_CODE_DESCRIPTIONS as Record<number, ExitCodeDescription>)[exitCode]
  return description?.docsAnchor
    ? `${TVM_EXIT_CODES_DOCS_BASE_URL}${description.docsAnchor}`
    : undefined
}
