import type {ExtendedContractABI} from "../api/compilerAbi"
import type {VerificationSourceResponse} from "../api/types"
import {normalizeCodeHash} from "./codeHash"
import type {
  CompilerAbiRegistration,
  ExplorerMetadataRegistry,
  RegisteredCompilerAbi,
  RegisteredSource,
  SourceRegistration,
} from "./types"

export function unverifiedSourceResponse(options: {
  readonly address?: string
  readonly codeHash?: string
}): VerificationSourceResponse {
  return {
    code_hash: normalizeCodeHash(options.codeHash) ?? "",
    verified: false,
    bundles: [],
  }
}

export class NullMetadataRegistry implements ExplorerMetadataRegistry {
  readonly canWriteAddressNames = false
  readonly canWriteCompilerAbis = false
  readonly canWriteSources = false

  async getAddressNames(addresses: readonly string[]): Promise<Record<string, string | undefined>> {
    return Object.fromEntries(addresses.map(address => [address, undefined]))
  }

  async setAddressName(): Promise<void> {
    throw new Error("Address names are read-only for this explorer.")
  }

  async getCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ExtendedContractABI | null>> {
    return Object.fromEntries(codeHashes.map(codeHash => [codeHash, null]))
  }

  async registerCompilerAbis(_entries: readonly CompilerAbiRegistration[]): Promise<void> {
    throw new Error("Compiler ABI registry is read-only for this explorer.")
  }

  async listCompilerAbis(): Promise<readonly RegisteredCompilerAbi[]> {
    return []
  }

  async deleteCompilerAbi(): Promise<void> {
    throw new Error("Compiler ABI registry is read-only for this explorer.")
  }

  async getSource(options: {
    readonly address?: string
    readonly codeHash?: string
  }): Promise<VerificationSourceResponse> {
    return unverifiedSourceResponse(options)
  }

  async registerSources(_entries: readonly SourceRegistration[]): Promise<void> {
    throw new Error("Source registry is read-only for this explorer.")
  }

  async listSources(): Promise<readonly RegisteredSource[]> {
    return []
  }

  async deleteSource(): Promise<void> {
    throw new Error("Source registry is read-only for this explorer.")
  }
}
