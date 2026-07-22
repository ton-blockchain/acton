import {SAMPLE_ADDRESS, type AbiRunGetMethod, type ContractABI} from "@acton/transaction-ui/abi"

export const GALLERY_CELL_BOC = "b5ee9c72010101010002000000"

const typeIndex = {
  bool: 5,
  coins: 6,
  address: 7,
  uint32: 9,
  addressOpt: 12,
  transferQuery: 13,
  transferQueries: 14,
  limits: 15,
  methodOptions: 16,
} as const

export const abiViewerGalleryAbi = {
  contract_name: "TreasuryRouter",
  compiler_name: "tolk",
  compiler_version: "1.4.2",
  version: "2",
  author: "Acton UI Gallery",
  description: "Routes treasury transfers and exposes read-only simulations through get methods.",
  declarations: [
    {
      kind: "struct",
      name: "TransferQuery",
      ty_idx: typeIndex.transferQuery,
      description: "One transfer included in a routing preview.",
      fields: [
        {name: "recipient", ty_idx: typeIndex.address},
        {name: "amount", ty_idx: typeIndex.coins},
        {name: "payload", ty_idx: 3},
      ],
    },
    {
      kind: "struct",
      name: "MethodOptions",
      ty_idx: typeIndex.methodOptions,
      description: "Nested input used to exercise complex get-method arguments.",
      fields: [
        {name: "urgent", ty_idx: typeIndex.bool},
        {name: "fallback", ty_idx: typeIndex.addressOpt},
        {name: "transfers", ty_idx: typeIndex.transferQueries},
        {name: "limits", ty_idx: typeIndex.limits},
      ],
    },
  ],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
    {kind: "intN", n: 32},
    {kind: "uintN", n: 32},
    {kind: "intN", n: 64},
    {kind: "uintN", n: 64},
    {kind: "addressOpt"},
    {kind: "StructRef", struct_name: "TransferQuery"},
    {kind: "arrayOf", inner_ty_idx: typeIndex.transferQuery},
    {kind: "mapKV", key_ty_idx: typeIndex.uint32, value_ty_idx: typeIndex.coins},
    {kind: "StructRef", struct_name: "MethodOptions"},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {storage_ty_idx: typeIndex.methodOptions},
  incoming_messages: [{body_ty_idx: typeIndex.transferQuery}],
  incoming_external: [],
  outgoing_messages: [],
  emitted_events: [],
  get_methods: [
    {
      name: "balanceOf",
      description: "Returns the balance available to an owner.",
      parameters: [{name: "owner", ty_idx: typeIndex.address}],
      return_ty_idx: typeIndex.coins,
      tvm_method_id: 107_486,
    },
    {
      name: "previewBatch",
      description: "Validates a nested transfer batch without changing contract state.",
      parameters: [{name: "options", ty_idx: typeIndex.methodOptions}],
      return_ty_idx: 1,
      tvm_method_id: 78_748,
    },
    {
      name: "quoteTransfer",
      description: "Quotes a transfer for one recipient and amount.",
      parameters: [
        {name: "recipient", ty_idx: typeIndex.address},
        {name: "amount", ty_idx: typeIndex.coins},
      ],
      return_ty_idx: typeIndex.coins,
      tvm_method_id: 96_241,
    },
    {
      name: "canTransfer",
      description: "Checks a transfer with an explicit urgent-mode flag.",
      parameters: [
        {name: "recipient", ty_idx: typeIndex.address},
        {name: "amount", ty_idx: typeIndex.coins},
        {name: "urgent", ty_idx: typeIndex.bool},
      ],
      return_ty_idx: typeIndex.bool,
      tvm_method_id: 96_242,
    },
    {
      name: "estimateRoute",
      description: "Estimates a route with several independently editable scalar arguments.",
      parameters: [
        {name: "sender", ty_idx: typeIndex.address},
        {name: "recipient", ty_idx: typeIndex.address},
        {name: "amount", ty_idx: typeIndex.coins},
        {name: "queryId", ty_idx: 11},
        {name: "urgent", ty_idx: typeIndex.bool},
        {name: "payload", ty_idx: 3},
      ],
      return_ty_idx: typeIndex.coins,
      tvm_method_id: 96_243,
    },
  ],
  thrown_errors: [],
} satisfies ContractABI

export const scalarMethodsAbi = {
  ...abiViewerGalleryAbi,
  get_methods: abiViewerGalleryAbi.get_methods.slice(0, 1),
} satisfies ContractABI

export const complexMethodsAbi = {
  ...abiViewerGalleryAbi,
  get_methods: abiViewerGalleryAbi.get_methods.slice(1, 2),
} satisfies ContractABI

export const argumentCountMethodsAbi = {
  ...abiViewerGalleryAbi,
  get_methods: abiViewerGalleryAbi.get_methods.slice(2),
} satisfies ContractABI

export const galleryAddressSuggestions = [
  {address: SAMPLE_ADDRESS, label: "Zero account"},
  {
    address: "EQAREREREREREREREREREREREREREREREREREREREREREeYT",
    label: "Treasury",
  },
] as const

export const runGalleryGetMethod: AbiRunGetMethod = async () => ({
  exit_code: 0,
  gas_used: 143,
  stack: [{type: "num", value: "1000000000"}],
  vm_log: "",
})
