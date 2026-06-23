/* eslint-disable */
/**
 * Representation of stack entry or group of stack entries
 */
export type StackEntry =
  | {
      type: "simple"
      name: VariableName
      range?: PossibleValueRange
      presentation?: string
      value_types?: PossibleValueTypes
      mutations?: Mutation[]
    }
  | {
      type: "const"
      value_type: ConstantType
      value: ConstantValue
    }
  | {
      type: "conditional"
      name: VariableName1
      match: MatchArm[]
      else?: StackValues
    }
  | {
      type: "array"
      name: VariableName
      length_var: VariableName2
      array_entry: ArraySingleEntryDefinition
    }
/**
 * Allowed chars are `a-zA-Z0-9_`, must not begin with digit or underscore and must not end with underscore.
 */
export type VariableName = string
export type PossibleValueTypes = (
  | "Int"
  | "Bool"
  | "Cell"
  | "Builder"
  | "Slice"
  | "Tuple"
  | "Continuation"
  | "Null"
  | "Any"
)[]
export type ConstantType = "Int" | "Null"
export type ConstantValue = number | string | null
/**
 * Allowed chars are `a-zA-Z0-9_`, must not begin with digit or underscore and must not end with underscore.
 */
export type VariableName1 = string
export type ArmValue = number
/**
 * Allowed chars are `a-zA-Z0-9_`, must not begin with digit or underscore and must not end with underscore.
 */
export type VariableName2 = string
/**
 * Array is a structure like `x1 y1 z1 x2 y2 z2 ... x_n y_n z_n n` which contains `n` entries of `x_i y_i z_i`. This property defines the structure of a single entry.
 */
export type ArraySingleEntryDefinition = StackEntry[]
/**
 * Stack constraints. Top of stack is the last value.
 */
export type StackValues = StackEntry[]
/**
 * Represents read/write access to a register
 */
export type Register =
  | {
      type: "constant"
      index: number
    }
  | {
      type: "variable"
      var_name: VariableName
    }
  | {
      type: "special"
      name: "gas" | "cstate" | "r"
    }
export type RegisterValues = Register[]
/**
 * Description of a continuation with static savelist
 */
export type Continuation =
  | {
      type: "cc"
      save?: ContinuationSavelist
    }
  | {
      type: "variable"
      var_name: ContinuationVariableName
      save?: ContinuationSavelist
    }
  | {
      type: "register"
      index: RegisterNumber03
      save?: ContinuationSavelist
    }
  | {
      type: "special"
      name: "until"
      args: {
        body: Continuation
        after: Continuation
      }
    }
  | {
      type: "special"
      name: "while"
      args: {
        cond: Continuation
        body: Continuation
        after: Continuation
      }
    }
  | {
      type: "special"
      name: "again"
      args: {
        body: Continuation
      }
    }
  | {
      type: "special"
      name: "repeat"
      args: {
        count: VariableName3
        body: Continuation
        after: Continuation
      }
    }
  | {
      type: "special"
      name: "pushint"
      args: {
        value: IntegerToPushToStack
        next: Continuation
      }
    }
/**
 * Allowed chars are `a-zA-Z0-9_`, must not begin with digit or underscore and must not end with underscore.
 */
export type ContinuationVariableName = string
export type RegisterNumber03 = number
/**
 * Allowed chars are `a-zA-Z0-9_`, must not begin with digit or underscore and must not end with underscore.
 */
export type VariableName3 = string
export type IntegerToPushToStack = number
/**
 * Array of current continuation possible values after current instruction execution
 */
export type PossibleBranchesOfAnInstruction = Continuation[]
/**
 * Argument for a Fift instruction
 */
export type FiftArgument = number | string

export interface Specification {
  $schema: string
  version: string
  instructions: Instruction[]
  fift_instructions: FiftInstruction[]
}

/**
 * Represents a TVM instruction with its properties, layout, effects and signature
 */
export interface Instruction {
  /**
   * The unique name of the instruction
   */
  name: string
  /**
   * Main category of the instruction (e.g. stack_basic, cont_loops, dict_get)
   */
  category: string
  /**
   * Sub-category for more specific grouping of instructions
   */
  sub_category: string
  description: Description
  layout: Layout
  /**
   * List of side effects that this instruction may have
   */
  effects?: string[]
  signature?: InstructionSignature
  control_flow?: ControlFlowOfInstruction
  implementation?: ImplementationInfo
}

/**
 * Detailed description of an instruction including documentation, examples and related information
 */
export interface Description {
  /**
   * Brief one-line description of the instruction
   */
  short: string
  /**
   * Detailed description of the instruction's functionality
   */
  long: string
  /**
   * List of operand names
   */
  operands: string[]
  /**
   * List of tags for categorizing and searching instructions
   */
  tags?: string[]
  /**
   * List of possible exit codes and their conditions
   */
  exit_codes?: ExitCode[]
  /**
   * List of alternative implementations of this instruction
   */
  other_implementations?: OtherImplementation[]
  /**
   * List of instructions that are related or similar to this one
   */
  related_instructions?: string[]
  /**
   * List of examples showing how to use this instruction
   */
  examples?: Example[]
  /**
   * List of gas consumption entries for this instruction
   */
  gas?: GasConsumptionEntry[]
  /**
   * List of documentation links related to this instruction
   */
  docs_links?: DocsLink[]
}

/**
 * Represents an instruction exit code and its condition
 */
export interface ExitCode {
  /**
   * Exit code number
   */
  errno: string
  /**
   * Condition that triggers this exit code
   */
  condition: string
}

/**
 * Alternative implementation of an instruction using other instructions
 */
export interface OtherImplementation {
  /**
   * Whether this implementation exactly matches the original instruction's behavior
   */
  exact: boolean
  /**
   * List of instructions that implement the same functionality
   */
  instructions: string[]
}

/**
 * Example of instruction usage with stack state and execution result
 */
export interface Example {
  /**
   * List of instructions in this example
   */
  instructions: ExampleInstruction[]
  stack: ExampleStack
  /**
   * Exit code of the example execution
   */
  exit_code?: number
}

/**
 * Single instruction in an example with optional comment and main flag
 */
export interface ExampleInstruction {
  /**
   * Instruction text or mnemonic
   */
  instruction: string
  /**
   * Optional comment explaining the instruction
   */
  comment?: string
  /**
   * Whether this is the main instruction being demonstrated
   */
  is_main?: boolean
}

/**
 * Stack state before and after instruction execution
 */
export interface ExampleStack {
  /**
   * Stack state before instruction execution
   */
  input: string[]
  /**
   * Stack state after instruction execution
   */
  output: string[]
}

/**
 * Represents gas consumption for an instruction with value and description
 */
export interface GasConsumptionEntry {
  /**
   * The gas consumption value
   */
  value: number
  /**
   * Description of the gas consumption
   */
  description: string
  /**
   * Optional formula for dynamic gas calculation
   */
  formula?: string
}

/**
 * Represents a link to documentation with name and URL
 */
export interface DocsLink {
  /**
   * Display name for the documentation link
   */
  name: string
  /**
   * URL to the documentation
   */
  url: string
}

/**
 * Information about instruction's bytecode layout and execution
 */
export interface Layout {
  /**
   * Minimum value for instruction operand range
   */
  min: number
  /**
   * Maximum value for instruction operand range
   */
  max: number
  checkLen: number
  skipLen: number
  args: Arg[]
  exec: string
  /**
   * Type of instruction layout format
   */
  kind: "ext" | "ext-range" | "fixed" | "fixed-range" | "simple"
  /**
   * Numeric value of instruction prefix
   */
  prefix: number
  /**
   * String representation of instruction prefix in hex
   */
  prefix_str: string
  /**
   * TLB schema of the instruction
   */
  tlb: string
  version?: number
}

export type Arg =
  | UintArg
  | IntArg
  | DeltaArg
  | StackArg
  | ControlArg
  | PlduzArg
  | TinyIntArg
  | LargeIntArg
  | MinusOneArg
  | S1Arg
  | SetcpArg
  | SliceArg
  | CodeSliceArg
  | Dict
  | RefCodeSliceArg
  | InlineCodeSliceArg
  | ExoticCellArg
  | DebugstrArg

/**
 * Value range constraint for the child argument
 */
export interface ArgRange {
  /**
   * Minimum value for the argument range
   */
  min: string
  /**
   * Maximum value for the argument range
   */
  max: string
}

export interface UintArg {
  $: "uint"
  /**
   * Length of the argument in bits
   */
  len: number
  range: ArgRange
}

export interface IntArg {
  $: "int"
  /**
   * Length of the argument in bits
   */
  len: number
  range: ArgRange
}

export interface DeltaArg {
  $: "delta"
  /**
   * Delta value
   */
  delta: number
  /**
   * Nested argument for delta operation
   */
  arg: Arg
}

export interface StackArg {
  $: "stack"
  /**
   * Length of the argument in bits
   */
  len: number
  range: ArgRange
}

export interface ControlArg {
  $: "control"
  range: ArgRange
}

export interface PlduzArg {
  $: "plduzArg"
  range: ArgRange
}

export interface TinyIntArg {
  $: "tinyInt"
  range: ArgRange
}

export interface LargeIntArg {
  $: "largeInt"
  range: ArgRange
}

export interface MinusOneArg {
  $: "minusOne"
}

export interface S1Arg {
  $: "s1"
}

export interface SetcpArg {
  $: "setcpArg"
  range: ArgRange
}

export interface SliceArg {
  $: "slice"
  /**
   * References argument for slice
   */
  refs: Arg
  /**
   * Bits argument for slice
   */
  bits: Arg
  /**
   * Padding value for slice
   */
  pad: number
}

export interface CodeSliceArg {
  $: "codeSlice"
  /**
   * References argument for code slice
   */
  refs: Arg
  /**
   * Bits argument for code slice
   */
  bits: Arg
}

export interface Dict {
  $: "dict"
}

export interface RefCodeSliceArg {
  $: "refCodeSlice"
}

export interface InlineCodeSliceArg {
  $: "inlineCodeSlice"
  /**
   * Bits argument for inline code slice
   */
  bits: Arg
}

export interface ExoticCellArg {
  $: "exoticCell"
}

export interface DebugstrArg {
  $: "debugstr"
}

/**
 * Information related to usage of stack and registers by instruction. If omitted, exact signature is not available.
 */
export interface InstructionSignature {
  stack_string?: string
  inputs?: InstructionInputs
  outputs?: InstructionOutputs
}

/**
 * Incoming values constraints.
 */
export interface InstructionInputs {
  stack?: StackValues
  registers?: RegisterValues
}

/**
 * Optional range constraint for the value, specifying minimum and maximum allowed values
 */
export interface PossibleValueRange {
  /**
   * Minimum allowed value (inclusive)
   */
  min: number
  /**
   * Maximum allowed value (inclusive)
   */
  max: number
}

export interface Mutation {
  length: {
    amount_arg?: number
    stack_amount_arg?: number
  }
}

export interface MatchArm {
  value: ArmValue
  stack: StackValues
}

/**
 * Outgoing values constraints.
 */
export interface InstructionOutputs {
  stack?: StackValues
  registers?: RegisterValues
}

/**
 * Information related to current cc modification by instruction
 */
export interface ControlFlowOfInstruction {
  branches: PossibleBranchesOfAnInstruction
}

/**
 * Values of saved control flow registers c0-c3
 */
export interface ContinuationSavelist {
  c0?: Continuation
  c1?: Continuation
  c2?: Continuation
  c3?: Continuation
}

/**
 * Information about the C++ implementation of the instruction in TON repository
 */
export interface ImplementationInfo {
  /**
   * Git commit hash of the TON repository
   */
  commit_hash: string
  /**
   * Path to the C++ file containing the implementation
   */
  file_path: string
  /**
   * Line number where the function is defined
   */
  line_number: number
  /**
   * Name of the C++ function implementing this instruction
   */
  function_name: string
}

/**
 * Represents a Fift instruction with its actual TVM instruction name and arguments
 */
export interface FiftInstruction {
  /**
   * The unique name of the Fift instruction
   */
  name: string
  /**
   * The actual TVM instruction name
   */
  actual_name: string
  /**
   * List of arguments for the instruction
   */
  arguments?: FiftArgument[]
  /**
   * Optional description of what the instruction does
   */
  description?: string
}
