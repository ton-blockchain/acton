import type {CompilerAbiLoader} from "../api/client"
import type {ExtendedContractABI} from "../api/compilerAbi"
import {NullMetadataRegistry} from "./nullRegistry"

export class BundledAbiRegistry extends NullMetadataRegistry {
  constructor(private readonly loadCompilerAbis: CompilerAbiLoader) {
    super()
  }

  override getCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ExtendedContractABI | null>> {
    return this.loadCompilerAbis(codeHashes)
  }
}
