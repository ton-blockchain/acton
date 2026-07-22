import type {ExtendedContractABI} from "../api/compilerAbi"
import type {VerificationSourceResponse} from "../api/types"

export interface CompilerAbiRegistration {
  readonly abi: ExtendedContractABI
}

export interface SourceRegistration {
  readonly codeHash: string
  readonly source: VerificationSourceResponse
}

export interface RegisteredCompilerAbi {
  readonly codeHash: string
  readonly abi: ExtendedContractABI
  readonly savedAt: number
}

export interface RegisteredSource {
  readonly codeHash: string
  readonly source: VerificationSourceResponse
  readonly savedAt: number
}

export interface ExplorerMetadataRegistry {
  readonly canWriteAddressNames: boolean
  readonly canWriteCompilerAbis: boolean
  readonly canWriteSources: boolean

  getAddressNames(addresses: readonly string[]): Promise<Record<string, string | undefined>>
  setAddressName(address: string, name: string | undefined): Promise<void>

  getCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ExtendedContractABI | null>>
  registerCompilerAbis(entries: readonly CompilerAbiRegistration[]): Promise<void>
  listCompilerAbis(): Promise<readonly RegisteredCompilerAbi[]>
  deleteCompilerAbi(codeHash: string): Promise<void>

  getSource(options: {
    readonly address?: string
    readonly codeHash?: string
  }): Promise<VerificationSourceResponse>
  registerSources(entries: readonly SourceRegistration[]): Promise<void>
  listSources(): Promise<readonly RegisteredSource[]>
  deleteSource(codeHash: string): Promise<void>
}
