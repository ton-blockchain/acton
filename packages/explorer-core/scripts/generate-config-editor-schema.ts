// Derive editor structure from the same generated TL-B types as the explorer.
// Keeping this mechanical avoids a second handwritten blockchain schema.
import {readFile, writeFile} from "node:fs/promises"

const input = await readFile(
  new URL("../src/cell-inspector/block.tlb.generated.ts", import.meta.url),
  "utf8",
)
const declarations = new Map<string, {fields?: string; alias?: string}>()
for (const match of input.matchAll(/export interface (\w+)(?:<[^>]+>)? \{([^}]+)\}/g)) {
  declarations.set(requiredCapture(match, 1), {fields: requiredCapture(match, 2)})
}
for (const match of input.matchAll(/export type (\w+)(?:<[^>]+>)? = ([^;]+);/g)) {
  declarations.set(requiredCapture(match, 1), {alias: requiredCapture(match, 2)})
}

type Shape = {type: string; [key: string]: unknown}
const definitions: Record<string, Shape> = {}

function requiredCapture(match: RegExpMatchArray, index: number): string {
  const value = match[index]
  if (value === undefined) throw new Error(`Generated TL-B source is missing capture ${index}`)

  return value
}

// Generated declarations use a deliberately small subset of TypeScript. Split
// only top-level separators so nested dictionaries and optional types stay intact.
function splitTypes(text: string, delimiter: string): string[] {
  let depth = 0
  let start = 0
  const result: string[] = []
  for (let i = 0; i < text.length; i++) {
    if (text[i] === "<") depth++
    if (text[i] === ">") depth--
    if (text[i] === delimiter && depth === 0) {
      result.push(text.slice(start, i).trim())
      start = i + 1
    }
  }
  result.push(text.slice(start).trim())
  return result
}

function shape(text: string): Shape {
  const union = splitTypes(text, "|")
  if (union.length > 1) return {type: "union", options: union.map(shape)}
  if (text.startsWith("'")) return {type: "literal", value: text.slice(1, -1)}
  if (["number", "bigint", "boolean", "undefined"].includes(text)) return {type: text}
  if (["Buffer", "Cell", "Slice", "BitString"].includes(text)) return {type: text.toLowerCase()}
  const generic = text.match(/^(\w+)<(.*)>$/)
  if (generic) {
    const genericName = requiredCapture(generic, 1)
    const args = splitTypes(requiredCapture(generic, 2), ",").map(shape)
    const first = args[0]
    const second = args[1]

    if (genericName === "Dictionary" && first && second) {
      return {type: "map", key: first, value: second}
    }
    if (genericName === "Maybe" && first) return {type: "maybe", value: first}

    throw new Error(`Unsupported config editor generic: ${text}`)
  }
  if (!/^\w+$/.test(text)) throw new Error(`Unsupported config editor type: ${text}`)
  define(text)
  return {type: "ref", name: text}
}

function define(name: string) {
  if (definitions[name]) return
  const node = declarations.get(name)
  if (!node) throw new Error(`Missing TL-B type ${name}`)
  definitions[name] = {type: "pending"}

  if (node.alias) {
    definitions[name] = shape(node.alias)
    return
  }
  if (!node.fields) throw new Error(`TL-B type ${name} has no fields or alias`)

  definitions[name] = {
    type: "struct",
    fields: Object.fromEntries(
      [...node.fields.matchAll(/readonly (\w+)\??: ([^;]+);/g)].map(member => [
        requiredCapture(member, 1),
        shape(requiredCapture(member, 2)),
      ]),
    ),
  }
}

define("ConfigParam")
const load = input.slice(
  input.indexOf("export function loadConfigParam("),
  input.indexOf("export function storeConfigParam("),
)
const parameters = Object.fromEntries(
  [...load.matchAll(/if \(\(arg0 == (-?\d+)\)\) \{([\s\S]*?)(?=\n {4}if |\n {4}throw )/g)].map(
    match => {
      const kind = match[2]?.match(/kind: '([^']+)'/)?.[1]
      if (!kind) throw new Error(`Missing constructor for parameter ${match[1]}`)
      return [requiredCapture(match, 1), kind]
    },
  ),
)

const output = {parameters, definitions}
await writeFile(
  new URL("../src/config/editor/schema.generated.json", import.meta.url),
  `${JSON.stringify(output, null, 2)}\n`,
)
