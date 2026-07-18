import {useEffect, useMemo, useState} from "react"
import type {ParsedCodeCell} from "@acton/ui"
import {Cell} from "@ton/core"

import {
  ContractSourcePanel,
  type ContractVerifiedSource,
} from "../ContractSourcePanel/ContractSourcePanel"

import styles from "./CodeCellDetails.module.css"

declare const Buffer: {
  from(value: string, encoding: "hex"): Parameters<typeof Cell.fromBoc>[0]
}

export type ResolveVerifiedSourceByCodeHash = (
  codeHash: string,
) => Promise<ContractVerifiedSource | undefined>

interface CodeCellDetailsProps {
  readonly cell: ParsedCodeCell
  readonly verifiedSourcesByCodeHash?: ReadonlyMap<string, ContractVerifiedSource>
  readonly resolveVerifiedSourceByCodeHash?: ResolveVerifiedSourceByCodeHash
}

export function CodeCellDetails({
  cell,
  verifiedSourcesByCodeHash,
  resolveVerifiedSourceByCodeHash,
}: CodeCellDetailsProps) {
  const code = useMemo(() => {
    try {
      const codeCell = Cell.fromBoc(Buffer.from(cell.bocHex, "hex"))[0]
      if (!codeCell) return undefined

      return {
        bocBase64: codeCell.toBoc({idx: false, crc32: false}).toString("base64"),
        hash: codeCell.hash().toString("hex"),
      }
    } catch {
      return undefined
    }
  }, [cell.bocHex])
  const cachedSource = code ? verifiedSourcesByCodeHash?.get(code.hash) : undefined
  const [lookupResult, setLookupResult] = useState<{
    readonly codeHash: string
    readonly source?: ContractVerifiedSource
  }>()
  const lookupSource =
    code && lookupResult?.codeHash === code.hash ? lookupResult.source : undefined

  useEffect(() => {
    if (!code || cachedSource || !resolveVerifiedSourceByCodeHash) return

    let active = true
    void resolveVerifiedSourceByCodeHash(code.hash).then(
      source => {
        if (active) setLookupResult({codeHash: code.hash, source})
      },
      () => {
        if (active) setLookupResult({codeHash: code.hash})
      },
    )

    return () => {
      active = false
    }
  }, [cachedSource, code, resolveVerifiedSourceByCodeHash])

  if (!code) {
    return <div className={styles.error}>Code cell could not be decoded</div>
  }

  return (
    <div className={styles.root}>
      <ContractSourcePanel
        codeBoc={code.bocBase64}
        defaultFileTreeVisible={false}
        verifiedSource={cachedSource ?? lookupSource}
        compact
      />
    </div>
  )
}
