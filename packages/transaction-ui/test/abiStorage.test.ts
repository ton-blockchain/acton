import {describe, expect, test} from "bun:test"

import {
  abiValueToFormValue,
  buildAbiStorageDataBoc,
  decodeAbiStorageDataBoc,
  getAbiStorageBuilderInfo,
  type ContractABI,
} from "../src/abi"

const storageAbi = {
  alias_instantiations: [],
  compiler_name: "tolk",
  compiler_version: "test",
  contract_name: "StorageBuilder",
  declarations: [
    {
      fields: [
        {name: "seqno", ty_idx: 9},
        {name: "enabled", ty_idx: 5},
        {name: "balance", ty_idx: 6},
      ],
      kind: "struct",
      name: "Storage",
      ty_idx: 12,
    },
  ],
  emitted_events: [],
  get_methods: [],
  incoming_external: [],
  incoming_messages: [],
  outgoing_messages: [],
  storage: {storage_ty_idx: 12},
  struct_instantiations: [],
  thrown_errors: [],
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
    {kind: "StructRef", struct_name: "Storage"},
  ],
} satisfies ContractABI

describe("ABI storage builder", () => {
  test("describes storage and round-trips edited JSON through BoC", () => {
    const info = getAbiStorageBuilderInfo(storageAbi)
    const storageJson = '{"seqno":"505","enabled":true,"balance":"1250000000"}'
    const boc = buildAbiStorageDataBoc(storageAbi, storageJson)
    const decoded = decodeAbiStorageDataBoc(storageAbi, boc)

    expect({
      info: info && {
        tyIdx: info.tyIdx,
        typeLabel: info.typeLabel,
        sample: JSON.parse(info.sampleJson),
      },
      boc,
      decoded: abiValueToFormValue(decoded),
    }).toMatchSnapshot()
    expect(buildAbiStorageDataBoc(storageAbi, JSON.stringify(abiValueToFormValue(decoded)))).toBe(
      boc,
    )
  })

  test("returns no builder info and rejects builds when storage is absent", () => {
    const abiWithoutStorage = {...storageAbi, storage: {}} satisfies ContractABI

    expect({
      missingAbi: getAbiStorageBuilderInfo(undefined),
      missingStorage: getAbiStorageBuilderInfo(abiWithoutStorage),
    }).toMatchSnapshot()
    expect(() => buildAbiStorageDataBoc(abiWithoutStorage, "{}")).toThrow(
      "ABI does not describe contract storage",
    )
    expect(() => decodeAbiStorageDataBoc(abiWithoutStorage, "00")).toThrow(
      "ABI does not describe contract storage",
    )
  })

  test("rejects malformed storage JSON and invalid field values", () => {
    expect(() => buildAbiStorageDataBoc(storageAbi, "{")).toThrow()
    expect(() =>
      buildAbiStorageDataBoc(storageAbi, '{"seqno":"nope","enabled":true,"balance":"0"}'),
    ).toThrow()
  })
})
