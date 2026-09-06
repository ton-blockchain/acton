import generated from "./tlb.generated.json"

/** Build-time schema catalog shared by the generator and the lazy TL-B viewer */
export interface ConfigTlbCatalog {
  readonly source: {readonly revision: string; readonly url: string; readonly sha256: string}
  readonly parameters: Readonly<
    Record<
      number,
      {
        readonly line: number
        readonly roots: readonly number[]
        readonly dependencies: readonly {
          readonly declaration: number
          readonly depth: number
          readonly parameterized: boolean
          readonly typeName: string
        }[]
      }
    >
  >
  readonly declarations: Readonly<Record<number, string>>
}

const catalog: ConfigTlbCatalog = generated

/** Identifies the pinned schema, independently of the block whose values are being viewed */
export const configTlbSource = catalog.source

/** Shows nearby concrete types, keeping booleans, containers and deeper implementation details optional */
export function getConfigParameterTlb(
  id: number,
  previewDepth = 2,
):
  | {readonly declaration: string; readonly dependencies: string; readonly sourceUrl: string}
  | undefined {
  const parameter = catalog.parameters[id]
  if (!parameter) return undefined

  const visible = [...parameter.roots]
  const hidden: number[] = []
  for (const dependency of parameter.dependencies) {
    const isBoolean = ["Bool", "True", "False"].includes(dependency.typeName)
    const target =
      !isBoolean && !dependency.parameterized && dependency.depth <= previewDepth ? visible : hidden
    target.push(dependency.declaration)
  }

  return {
    sourceUrl: `${catalog.source.url}#L${parameter.line}`,
    declaration: visible.map(index => catalog.declarations[index]).join("\n\n"),
    dependencies: hidden.map(index => catalog.declarations[index]).join("\n\n"),
  }
}
