import {Popover} from "@acton/ui"
import type {ABIGetMethod, SymTable} from "@ton/tolk-abi-to-typescript"

import {
  formatAbiTyDeclaration,
  formatGetMethodSignature,
  formatTolkIdentifier,
  formatType,
  tryTyByIdx,
} from "./abiFormatting"
import {TolkCode} from "./abiShared"
import styles from "./AbiViewer.module.css"

export function AbiMethodSignature({
  method,
  symbols,
}: {
  readonly method: ABIGetMethod
  readonly symbols: SymTable
}) {
  return (
    <code className={styles.methodSignature} aria-label={formatGetMethodSignature(method, symbols)}>
      <span className={styles.signatureKeyword}>get fun</span>{" "}
      <span className={styles.signatureName}>{formatTolkIdentifier(method.name)}</span>
      <span className={styles.signaturePunctuation}>(</span>
      {method.parameters.map((parameter, index) => (
        <span key={`${parameter.name}:${index}`}>
          {index > 0 && <span className={styles.signaturePunctuation}>, </span>}
          <span className={styles.signatureParameter}>{formatTolkIdentifier(parameter.name)}</span>
          <span className={styles.signaturePunctuation}>: </span>
          <AbiTypeToken symbols={symbols} tyIdx={parameter.ty_idx} />
        </span>
      ))}
      <span className={styles.signaturePunctuation}>): </span>
      <AbiTypeToken symbols={symbols} tyIdx={method.return_ty_idx} />
    </code>
  )
}

function AbiTypeToken({symbols, tyIdx}: {readonly symbols: SymTable; readonly tyIdx: number}) {
  const typeName = formatType(symbols, tyIdx)
  const definition = formatAbiTyDeclaration(symbols, tyIdx)
  const hasExpandedDefinition = definition.trim() !== typeName.trim()
  const ty = tryTyByIdx(symbols, tyIdx)
  const isNamedType = ty?.kind === "StructRef" || ty?.kind === "AliasRef" || ty?.kind === "EnumRef"
  const className = `${styles.signatureType} ${isNamedType ? styles.signatureNamedType : ""}`

  if (!hasExpandedDefinition) {
    return <span className={className}>{typeName}</span>
  }

  return (
    <Popover
      triggerAsChild
      content={<TolkCode value={definition} wrap />}
      placement="bottom"
      maxWidth="min(560px, calc(100vw - 32px))"
      ariaLabel={`Definition of ${typeName}`}
    >
      <button type="button" className={`${className} ${styles.signatureTypeInteractive}`}>
        {typeName}
      </button>
    </Popover>
  )
}
