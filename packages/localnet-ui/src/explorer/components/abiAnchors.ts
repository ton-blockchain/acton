export type AbiSymbolAnchorKind = "declaration" | "error" | "get-method" | "message" | "storage"

export function abiSymbolAnchorId(
  kind: AbiSymbolAnchorKind,
  name: string,
  suffix?: string,
): string {
  const slug = [name, suffix]
    .filter((part): part is string => Boolean(part))
    .join("-")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")

  return `abi-${kind}-${slug || "symbol"}`
}
