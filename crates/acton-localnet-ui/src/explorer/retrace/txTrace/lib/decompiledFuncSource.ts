import type {SourceTraceResponse, TolkSourceMapData, TraceResult} from "@ton/retracer-core"
import {buildSourceTraceForTraceResult} from "@ton/retracer-core"

import type {VerificationSourceResponse} from "../../../api/types"

interface DecompiledFuncArtifacts extends TolkSourceMapData {
  readonly funcCode: string
  readonly codeHashHex: string
}

export interface DecompiledFuncSourceTrace {
  readonly source: VerificationSourceResponse
  readonly trace: SourceTraceResponse
  readonly bundleHash: string
}

type DecompilerWasmModule = typeof import("tasm-web-wasm")

let decompilerModulePromise: Promise<DecompilerWasmModule> | undefined

async function loadDecompilerModule(): Promise<DecompilerWasmModule> {
  decompilerModulePromise ??= import("tasm-web-wasm")
    .then(async module => {
      await module.default()
      return module
    })
    .catch(error => {
      decompilerModulePromise = undefined
      throw error
    })

  return decompilerModulePromise
}

async function decompileResultCode(
  result: TraceResult,
): Promise<DecompiledFuncArtifacts | undefined> {
  const codeCell = result.codeCell
  if (!codeCell) {
    return undefined
  }

  const module = await loadDecompilerModule()
  return module.decompile_boc_bytes_with_source_map(
    codeCell.toBoc({idx: false, crc32: false}),
    false,
  ) as DecompiledFuncArtifacts
}

export async function buildDecompiledFuncSourceTrace(
  result: TraceResult,
): Promise<DecompiledFuncSourceTrace | undefined> {
  try {
    const artifacts = await decompileResultCode(result)
    if (!artifacts) {
      return undefined
    }

    const codeHash = artifacts.codeHashHex.toLowerCase()
    const bundleHash = `decompiled-func:${codeHash}`
    const sourceMap = {
      code_boc64: artifacts.codeBoc64,
      symbol_types_json: artifacts.symbolTypesJson,
      debug_marks_json: artifacts.debugMarksJson,
      debug_marks_base64: artifacts.debugMarksBase64,
    }

    return {
      bundleHash,
      trace: await buildSourceTraceForTraceResult(result, artifacts),
      source: {
        code_hash: codeHash,
        verified: false,
        bundles: [
          {
            source_bundle_hash: bundleHash,
            verified_at: 0,
            storage_revision: "decompiled",
            entrypoint: "decompiled.fc",
            compiler: {
              language: "func",
              version: "tasm-web-wasm 0.1.0",
              params: {generated: true},
            },
            files: [
              {
                path: "decompiled.fc",
                content_hash: codeHash,
                include_in_command: false,
                is_stdlib: false,
                has_include_directives: false,
                content: artifacts.funcCode,
              },
            ],
            source_map: sourceMap,
          },
        ],
      },
    }
  } catch (error) {
    // Source-level fallback must never break the existing assembler debugger.
    console.debug("Failed to build decompiled FunC source trace", error)
    return undefined
  }
}
