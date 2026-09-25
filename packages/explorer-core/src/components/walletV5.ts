import {Address, Cell, Dictionary} from "@ton/core"

export interface WalletV5Storage {
  readonly isSignatureAllowed: boolean
  readonly seqno: number
  readonly walletId: number
  readonly extensions: readonly string[]
}

/** Reads Wallet V5 R1 storage; missing storage should be handled by the caller. */
export function parseWalletV5Storage(dataBoc: string, walletAddress: string): WalletV5Storage {
  const {workChain} = Address.parse(walletAddress)

  try {
    const root = Cell.fromBase64(dataBoc)
    // Dictionary.load skips exotic branches, which would otherwise look like absent plugins.
    const pending = [root]
    const visited = new Set<Cell>()
    while (pending.length > 0) {
      const cell = pending.pop()
      if (!cell || visited.has(cell)) continue
      if (cell.isExotic) throw new TypeError("Incomplete Wallet V5 dictionary.")
      visited.add(cell)
      pending.push(...cell.refs)
    }
    const storage = root.beginParse()
    const isSignatureAllowed = storage.loadBit()
    const seqno = storage.loadUint(32)
    const walletId = storage.loadUint(32)
    storage.skip(256) // Ed25519 public key.
    const extensions = storage.loadDict(Dictionary.Keys.Buffer(32), Dictionary.Values.Bool())
    storage.endParse()

    return {
      isSignatureAllowed,
      seqno,
      walletId,
      // V5 checks dictionary membership, regardless of the stored boolean value.
      // Its add-extension action requires the extension to share the wallet's workchain.
      extensions: extensions
        .keys()
        .map(hash => new Address(workChain, hash).toRawString())
        .sort(),
    }
  } catch {
    throw new TypeError("Wallet V5 storage is invalid or incomplete.")
  }
}
