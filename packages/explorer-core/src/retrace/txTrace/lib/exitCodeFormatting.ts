import {getStandardExitCodeInfo} from "@acton/ui"
import type {ABIThrownError, ContractABI} from "@ton/tolk-abi-to-typescript"

export interface ExitCodeFormatOptions {
  readonly compilerAbi?: ContractABI
  readonly vmDescription?: string | null
}

export interface FormattedExitCode {
  readonly code: string
  readonly description: string
  readonly title: string
  readonly tooltip: string
}

function normalizedText(value: string | null | undefined): string | undefined {
  const trimmed = value?.trim()
  return trimmed && trimmed.length > 0 ? trimmed : undefined
}

function numericExitCode(code: number | string): number | undefined {
  const numeric = typeof code === "number" ? code : Number(code.trim())
  return Number.isFinite(numeric) ? numeric : undefined
}

function findAbiThrownError(
  compilerAbi: ContractABI | undefined,
  code: number | string,
): ABIThrownError | undefined {
  const numeric = numericExitCode(code)
  if (numeric === undefined) {
    return undefined
  }

  return compilerAbi?.thrown_errors.find(error => error.err_code === numeric)
}

function exitCodeDescription(code: number | string, options: ExitCodeFormatOptions): string {
  const abiError = findAbiThrownError(options.compilerAbi, code)
  const abiDescription = normalizedText(abiError?.description)
  if (abiDescription) {
    return abiDescription
  }

  const abiName = normalizedText(abiError?.name)
  if (abiName) {
    return abiName
  }

  const vmDescription = normalizedText(options.vmDescription)
  if (vmDescription) {
    return vmDescription
  }

  const numeric = numericExitCode(code)
  const stdInfo = numeric === undefined ? undefined : getStandardExitCodeInfo(numeric)
  if (stdInfo?.name) {
    return stdInfo.name
  }

  return "Custom error"
}

export function formatExitCode(
  code: number | string,
  options: ExitCodeFormatOptions = {},
): FormattedExitCode {
  const codeLabel = String(code).trim() || "unknown"
  const description = exitCodeDescription(code, options)
  const message = `Exit Code: ${codeLabel}: ${description}`

  return {
    code: codeLabel,
    description,
    title: `⚡ ${message}`,
    tooltip: `Transaction failed with ${message}`,
  }
}
