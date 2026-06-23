export const CONTROL_REGISTERS: Readonly<
  Record<string, {readonly type: string; readonly description: string}>
> = {
  c0: {
    type: "Continuation",
    description: "Stores the next or return continuation, similar to a return address.",
  },
  c1: {
    type: "Continuation",
    description: "Stores the alternative continuation.",
  },
  c2: {
    type: "Continuation",
    description: "Contains the exception handler (continuation).",
  },
  c3: {
    type: "Continuation",
    description: "Holds the current dictionary (hashmap of function codes) as a continuation.",
  },
  c4: {
    type: "Cell",
    description: "Stores persistent data (contract's data section).",
  },
  c5: {
    type: "Cell",
    description: "Contains output actions.",
  },
  c7: {
    type: "Tuple",
    description: "Stores temporary data.",
  },
} as const
