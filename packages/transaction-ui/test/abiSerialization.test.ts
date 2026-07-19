import {describe, expect, test} from "bun:test"
import {Address, beginCell, Dictionary} from "@ton/core"

import {
  SAMPLE_ADDRESS,
  SAMPLE_EXTERNAL_ADDRESS,
  TON_ADDR_NONE,
  abiValueToFormValue,
  createAbiSymbols,
  decodeAbiValueFromBoc,
  encodeAbiValueToBoc,
  formatAbiCellBoc,
  isTonAddress,
  parseAbiCellArg,
  sampleAbiValueForTy,
  stringifyAbiJson,
  type ContractABI,
} from "../src/abi"

const nestedAbi = {
  contract_name: "AbiRoundTrip",
  compiler_name: "tolk",
  compiler_version: "test",
  declarations: [
    {
      kind: "struct",
      name: "ScalarFields",
      ty_idx: 14,
      fields: [
        {name: "count", ty_idx: 8},
        {name: "enabled", ty_idx: 5},
        {name: "recipient", ty_idx: 7},
        {name: "tonAmount", ty_idx: 6},
        {name: "maybeAddress", ty_idx: 12},
        {name: "maybeCount", ty_idx: 13},
      ],
    },
    {
      kind: "alias",
      name: "CounterAlias",
      ty_idx: 15,
      target_ty_idx: 8,
    },
    {
      kind: "struct",
      name: "NestedStruct",
      ty_idx: 16,
      fields: [
        {name: "summary", ty_idx: 14},
        {name: "counterAlias", ty_idx: 15},
      ],
    },
    {
      kind: "struct",
      name: "ComplexCollections",
      ty_idx: 20,
      fields: [
        {name: "records", ty_idx: 17},
        {name: "recordsByBatch", ty_idx: 18},
        {name: "nestedBatches", ty_idx: 19},
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
    {kind: "nullable", inner_ty_idx: 8},
    {kind: "StructRef", struct_name: "ScalarFields"},
    {kind: "AliasRef", alias_name: "CounterAlias"},
    {kind: "StructRef", struct_name: "NestedStruct"},
    {kind: "arrayOf", inner_ty_idx: 14},
    {kind: "mapKV", key_ty_idx: 9, value_ty_idx: 17},
    {kind: "arrayOf", inner_ty_idx: 18},
    {kind: "StructRef", struct_name: "ComplexCollections"},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {storage_ty_idx: 16},
  incoming_messages: [],
  incoming_external: [],
  outgoing_messages: [],
  emitted_events: [],
  get_methods: [],
  thrown_errors: [],
} satisfies ContractABI

const nestedFormValue = {
  summary: {
    count: "7",
    enabled: true,
    recipient: SAMPLE_ADDRESS,
    tonAmount: "1250000000",
    maybeAddress: null,
    maybeCount: "42",
  },
  counterAlias: "99",
}

const complexCollectionFormValue = {
  records: [nestedFormValue.summary],
  recordsByBatch: {"1": [nestedFormValue.summary]},
  nestedBatches: [{"7": [nestedFormValue.summary]}],
}

const dictionaryAbi = {
  contract_name: "DictionaryRoundTrip",
  compiler_name: "tolk",
  compiler_version: "test",
  declarations: [],
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
    {kind: "mapKV", key_ty_idx: 9, value_ty_idx: 6},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {storage_ty_idx: 10},
  incoming_messages: [],
  incoming_external: [],
  outgoing_messages: [],
  emitted_events: [],
  get_methods: [],
  thrown_errors: [],
} satisfies ContractABI

const addressAbi = {
  contract_name: "AddressRoundTrip",
  compiler_name: "tolk",
  compiler_version: "test",
  declarations: [],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
    {kind: "addressExt"},
    {kind: "addressAny"},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {storage_ty_idx: 7},
  incoming_messages: [],
  incoming_external: [],
  outgoing_messages: [],
  emitted_events: [],
  get_methods: [],
  thrown_errors: [],
} satisfies ContractABI

const fixedBitsAbi = {
  contract_name: "FixedBitsRoundTrip",
  compiler_name: "tolk",
  compiler_version: "test",
  declarations: [],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
    {kind: "bitsN", n: 512},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {storage_ty_idx: 8},
  incoming_messages: [],
  incoming_external: [],
  outgoing_messages: [],
  emitted_events: [],
  get_methods: [],
  thrown_errors: [],
} satisfies ContractABI

describe("ABI value serialization", () => {
  test("round-trips nested form values through Cell and BoC", () => {
    const boc = encodeAbiValueToBoc(nestedAbi, 16, nestedFormValue)
    const decoded = decodeAbiValueFromBoc(nestedAbi, 16, boc)
    const decodedFormValue = abiValueToFormValue(decoded)

    expect({boc, decoded: decodedFormValue}).toMatchSnapshot()
    expect(encodeAbiValueToBoc(nestedAbi, 16, decoded)).toBe(boc)
  })

  test("round-trips dictionaries with numeric keys and coins values", () => {
    const formValue = {"1": "1000000000", "7": "250000000"}
    const boc = encodeAbiValueToBoc(dictionaryAbi, 10, formValue)
    const decoded = decodeAbiValueFromBoc(dictionaryAbi, 10, boc)

    expect({boc, decoded: abiValueToFormValue(decoded)}).toMatchSnapshot()
  })

  test("round-trips nested collections with struct values", () => {
    const boc = encodeAbiValueToBoc(nestedAbi, 20, complexCollectionFormValue)
    const decoded = decodeAbiValueFromBoc(nestedAbi, 20, boc)

    expect({boc, decoded: abiValueToFormValue(decoded)}).toMatchSnapshot()
    expect(encodeAbiValueToBoc(nestedAbi, 20, decoded)).toBe(boc)
  })

  test("formats TON runtime values as JSON-safe form values", () => {
    const cell = beginCell().storeUint(0xab, 8).endCell()
    const dictionary = Dictionary.empty(Dictionary.Keys.Uint(8), Dictionary.Values.BigUint(16))
    dictionary.set(7, 1024n)

    const value = {
      address: Address.parse(SAMPLE_ADDRESS),
      integer: 42n,
      cell,
      slice: cell.beginParse(),
      builder: cell.asBuilder(),
      dictionary,
    }

    expect(abiValueToFormValue(value)).toMatchSnapshot()
    expect(stringifyAbiJson(value)).toBe(JSON.stringify(abiValueToFormValue(value), null, 2))
  })

  test("accepts equivalent hexadecimal and base64 BoCs", () => {
    const cell = beginCell().storeUint(0xab, 8).endCell()
    const hex = formatAbiCellBoc(cell)
    const base64 = cell.toBoc().toString("base64")

    expect(parseAbiCellArg(hex).equals(parseAbiCellArg(base64))).toBe(true)
    expect(parseAbiCellArg(`0x${hex}`).equals(cell)).toBe(true)
  })

  test("creates a serializable zero-filled default for fixed-width bits", () => {
    const sample = sampleAbiValueForTy(createAbiSymbols(fixedBitsAbi), 8)
    const sampleSlice = parseAbiCellArg(String(sample)).beginParse()
    const boc = encodeAbiValueToBoc(fixedBitsAbi, 8, sample)

    expect({
      sample,
      sampleBits: sampleSlice.remainingBits,
      sampleRefs: sampleSlice.remainingRefs,
      boc,
      decoded: abiValueToFormValue(decodeAbiValueFromBoc(fixedBitsAbi, 8, boc)),
    }).toMatchSnapshot()
  })

  test("round-trips external and addr_none address values", () => {
    const externalBoc = encodeAbiValueToBoc(addressAbi, 8, SAMPLE_EXTERNAL_ADDRESS)
    const noneBoc = encodeAbiValueToBoc(addressAbi, 9, TON_ADDR_NONE)

    expect({
      externalBoc,
      external: abiValueToFormValue(decodeAbiValueFromBoc(addressAbi, 8, externalBoc)),
      noneBoc,
      none: abiValueToFormValue(decodeAbiValueFromBoc(addressAbi, 9, noneBoc)),
    }).toMatchSnapshot()
  })

  test("validates address kinds without rewriting the input", () => {
    expect({
      internal: isTonAddress(SAMPLE_ADDRESS, "internal"),
      externalAsInternal: isTonAddress(SAMPLE_EXTERNAL_ADDRESS, "internal"),
      external: isTonAddress(SAMPLE_EXTERNAL_ADDRESS, "external"),
      noneAsAny: isTonAddress(TON_ADDR_NONE, "any"),
      noneAsInternal: isTonAddress(TON_ADDR_NONE, "internal"),
      invalidExternalWidth: isTonAddress("External<8:256>", "external"),
    }).toMatchSnapshot()
  })
})
