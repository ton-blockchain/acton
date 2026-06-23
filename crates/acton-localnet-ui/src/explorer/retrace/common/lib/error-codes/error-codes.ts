export const EXIT_CODE_DESCRIPTIONS = {
  0: {
    name: "Success",
    description: "Standard successful execution exit code.",
    phase: "Compute and action phases",
  },
  1: {
    name: "Alt Success",
    description: "Alternative successful execution exit code. Reserved, but does not occur.",
    phase: "Compute phase",
  },
  2: {
    name: "Stack Underflow",
    description: "Stack underflow.",
    phase: "Compute phase",
  },
  3: {
    name: "Stack Overflow",
    description: "Stack overflow.",
    phase: "Compute phase",
  },
  4: {
    name: "Integer Overflow",
    description: "Integer overflow.",
    phase: "Compute phase",
  },
  5: {
    name: "Range Check Error",
    description: "Range check error — an integer is out of its expected range.",
    phase: "Compute phase",
  },
  6: {
    name: "Invalid Opcode",
    description: "Invalid TVM opcode.",
    phase: "Compute phase",
  },
  7: {
    name: "Type Check Error",
    description: "Type check error.",
    phase: "Compute phase",
  },
  8: {
    name: "Cell Overflow",
    description: "Cell overflow.",
    phase: "Compute phase",
  },
  9: {
    name: "Cell Underflow",
    description: "Cell underflow.",
    phase: "Compute phase",
  },
  10: {
    name: "Dictionary Error",
    description: "Dictionary error.",
    phase: "Compute phase",
  },
  11: {
    name: "Unknown Error",
    description: "Unknown error, may be thrown by user programs.",
    phase: "Compute phase",
  },
  12: {
    name: "Fatal Error",
    description: "Fatal error. Thrown by TVM in situations deemed impossible.",
    phase: "Compute phase",
  },
  13: {
    name: "Out of Gas",
    description: "Out of gas error.",
    phase: "Compute phase",
  },
  [-14]: {
    name: "Out of Gas (Negative)",
    description: "Same as 13. Negative, so that it cannot be faked.",
    phase: "Compute phase",
  },
  14: {
    name: "VM Virtualization",
    description: "VM virtualization error. Reserved, but never thrown.",
    phase: "Compute phase",
  },
  32: {
    name: "Invalid Action List",
    description: "Action list is invalid.",
    phase: "Action phase",
  },
  33: {
    name: "Action List Too Long",
    description: "Action list is too long.",
    phase: "Action phase",
  },
  34: {
    name: "Invalid Action",
    description: "Action is invalid or not supported.",
    phase: "Action phase",
  },
  35: {
    name: "Invalid Source Address",
    description: "Invalid source address in outbound message.",
    phase: "Action phase",
  },
  36: {
    name: "Invalid Destination Address",
    description: "Invalid destination address in outbound message.",
    phase: "Action phase",
  },
  37: {
    name: "Not Enough Toncoin",
    description: "Not enough Toncoin.",
    phase: "Action phase",
  },
  38: {
    name: "Not Enough Extra Currencies",
    description: "Not enough extra currencies.",
    phase: "Action phase",
  },
  39: {
    name: "Message Too Large",
    description: "Outbound message does not fit into a cell after rewriting.",
    phase: "Action phase",
  },
  40: {
    name: "Cannot Process Message",
    description:
      "Cannot process a message — not enough funds, the message is too large, or its Merkle depth is too big.",
    phase: "Action phase",
  },
  41: {
    name: "Library Reference Null",
    description: "Library reference is null during library change action.",
    phase: "Action phase",
  },
  42: {
    name: "Library Change Error",
    description: "Library change action error.",
    phase: "Action phase",
  },
  43: {
    name: "Library Limits Exceeded",
    description:
      "Exceeded the maximum number of cells in the library or the maximum depth of the Merkle tree.",
    phase: "Action phase",
  },
  50: {
    name: "Account Size Exceeded",
    description: "Account state size exceeded limits.",
    phase: "Action phase",
  },
  128: {
    name: "Null Reference",
    description: "Null reference exception. Configurable since Tact 1.6.",
    phase: "Tact compiler (Compute phase)",
  },
  129: {
    name: "Invalid Serialization",
    description: "Invalid serialization prefix.",
    phase: "Tact compiler (Compute phase)",
  },
  130: {
    name: "No Receiver",
    description:
      "Invalid incoming message — there is no receiver for the opcode of the received message.",
    phase: "Tact compiler (Compute phase)",
  },
  131: {
    name: "Constraints Error",
    description: "Constraints error. Reserved, but never thrown.",
    phase: "Tact compiler (Compute phase)",
  },
  132: {
    name: "Access Denied",
    description: "Access denied — someone other than the owner sent a message to the contract.",
    phase: "Tact compiler (Compute phase)",
  },
  133: {
    name: "Contract Stopped",
    description: "Contract stopped.",
    phase: "Tact compiler (Compute phase)",
  },
  134: {
    name: "Invalid Argument",
    description: "Invalid argument.",
    phase: "Tact compiler (Compute phase)",
  },
  135: {
    name: "Code Not Found",
    description: "Code of a contract was not found.",
    phase: "Tact compiler (Compute phase)",
  },
  136: {
    name: "Invalid Address",
    description: "Invalid standard address.",
    phase: "Tact compiler (Compute phase)",
  },
  137: {
    name: "No Masterchain Support",
    description: "Masterchain support is not enabled for this contract. Removed since Tact 1.6.",
    phase: "Tact compiler (Compute phase)",
  },
  138: {
    name: "Not Basechain Address",
    description: "Not a basechain address. Available since Tact 1.6.3.",
    phase: "Tact compiler (Compute phase)",
  },
} as const
