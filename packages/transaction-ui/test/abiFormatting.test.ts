import {describe, expect, test} from "bun:test"
import {DynamicCtx, type ContractABI} from "@ton/tolk-abi-to-typescript"

import {
  formatAbiTyDeclaration,
  formatDeclarationName,
  formatDeclarationTolk,
  formatGetMethodSignature,
  formatType,
} from "../src/components/AbiViewer/abiFormatting"
import fixture from "./fixtures/abiRendering.json" with {type: "json"}

// Generated from fixtures/abiRendering.tolk by the real Tolk compiler.
const abi = fixture as ContractABI
const {symbols} = new DynamicCtx(abi)

describe("ABI rendering of compiled Tolk types", () => {
  test("specializes storage fields, including nested references to the same generic", () => {
    expect(
      Object.entries(abi.storage)
        .map(([role, tyIdx]) => `${role}:\n${formatAbiTyDeclaration(symbols, tyIdx)}`)
        .join("\n\n"),
    ).toMatchSnapshot()
  })

  test("renders concrete messages, generic aliases, union tags and wide prefixes", () => {
    expect(
      (["incoming_messages", "incoming_external", "outgoing_messages", "emitted_events"] as const)
        .map(
          group =>
            `${group}:\n${abi[group].map(message => formatAbiTyDeclaration(symbols, message.body_ty_idx)).join("\n\n")}`,
        )
        .join("\n\n"),
    ).toMatchSnapshot()
  })

  test("preserves template parameters, enum encoding, defaults and client serialization metadata", () => {
    expect(
      abi.declarations
        .map(
          declaration =>
            `${formatDeclarationName(declaration, symbols)}:\n${formatDeclarationTolk(declaration, symbols)}`,
        )
        .join("\n\n"),
    ).toMatchSnapshot()
  })

  test("renders getter signatures and concrete type definitions consistently", () => {
    expect(
      abi.get_methods
        .map(method =>
          [
            formatGetMethodSignature(method, symbols),
            ...method.parameters.map(
              parameter =>
                `${parameter.name}:\n${formatAbiTyDeclaration(symbols, parameter.ty_idx)}`,
            ),
            `result:\n${formatAbiTyDeclaration(symbols, method.return_ty_idx)}`,
          ].join("\n\n"),
        )
        .join("\n\n"),
    ).toMatchSnapshot()
  })

  test("preserves compound Tolk syntax and escaped identifiers at every nesting level", () => {
    expect(
      abi.unique_types.map((_, index) => formatType(symbols, index)).join("\n"),
    ).toMatchSnapshot()
  })

  test("does not substitute concrete arguments into later template renders", () => {
    const storage = abi.declarations.find(declaration => declaration.name === "Storage")!
    const before = formatDeclarationTolk(storage, symbols)
    const originalAbi = structuredClone(abi)
    formatAbiTyDeclaration(symbols, abi.storage.storage_ty_idx!)
    formatAbiTyDeclaration(symbols, abi.storage.storage_at_deployment_ty_idx!)
    expect(formatDeclarationTolk(storage, symbols)).toBe(before)
    expect(abi).toEqual(originalAbi)
  })
})
