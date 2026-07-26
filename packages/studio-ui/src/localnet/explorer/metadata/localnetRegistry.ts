import type {TonClient} from "../api/client"
import type {ExtendedContractABI} from "../api/compilerAbi"
import type {VerificationSourceResponse} from "../api/types"
import {unverifiedSourceResponse} from "./nullRegistry"
import type {
  CompilerAbiRegistration,
  ExplorerMetadataRegistry,
  RegisteredCompilerAbi,
  RegisteredSource,
  SourceRegistration,
} from "./types"

export class LocalnetMetadataRegistry implements ExplorerMetadataRegistry {
  readonly canWriteAddressNames = true
  readonly canWriteCompilerAbis = true
  readonly canWriteSources = true

  constructor(private readonly client: TonClient) {}

  getAddressNames(addresses: readonly string[]): Promise<Record<string, string | undefined>> {
    return this.client.getAddressNames(addresses)
  }

  setAddressName(address: string, name: string | undefined): Promise<void> {
    return this.client.setAddressName(address, name ?? "")
  }

  getCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ExtendedContractABI | null>> {
    return this.client.getRegisteredCompilerAbis(codeHashes)
  }

  registerCompilerAbis(entries: readonly CompilerAbiRegistration[]): Promise<void> {
    return this.client.registerCompilerAbis(entries)
  }

  listCompilerAbis(): Promise<readonly RegisteredCompilerAbi[]> {
    return this.client.listRegisteredCompilerAbis()
  }

  deleteCompilerAbi(codeHash: string): Promise<void> {
    return this.client.deleteRegisteredCompilerAbi(codeHash)
  }

  async getSource(options: {
    readonly address?: string
    readonly codeHash?: string
  }): Promise<VerificationSourceResponse> {
    return this.client
      .getRegisteredVerifiedSource(options)
      .catch(() => unverifiedSourceResponse(options))
  }

  registerSources(entries: readonly SourceRegistration[]): Promise<void> {
    return this.client.registerVerifiedSources(entries)
  }

  listSources(): Promise<readonly RegisteredSource[]> {
    return this.client.listRegisteredVerifiedSources()
  }

  deleteSource(codeHash: string): Promise<void> {
    return this.client.deleteRegisteredVerifiedSource(codeHash)
  }
}
