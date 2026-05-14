import fs from "node:fs"
import path from "node:path"

const MAX_LINE_LENGTH = 100
const ROOTS = ["content/docs", "../src/doc/man"]
const INCLUDED_EXTENSIONS = new Set([".md", ".mdx"])
const EXCLUDED_PATH_SEGMENTS = [
  "content/docs/standard_library/",
  "content/docs/tolk_standard_library/",
  "content/docs/rules/",
]
const EXCLUDED_LINE_PATTERNS = [
  "curl -LsSf https://github.com/ton-blockchain/acton/releases/latest/download/acton-installer.sh | sh",
  "te6ccg",
]

type Violation = {
  file: string
  line: number
  length: number
  content: string
}

function walk(dir: string): string[] {
  const entries = fs.readdirSync(dir, {withFileTypes: true})
  const files: string[] = []

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      files.push(...walk(fullPath))
      continue
    }

    if (INCLUDED_EXTENSIONS.has(path.extname(entry.name))) {
      files.push(fullPath)
    }
  }

  return files
}

function collectViolations(file: string): Violation[] {
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/)
  const violations: Violation[] = []

  let inFence = false

  for (let index = 0; index < lines.length; index++) {
    const line = lines[index]
    const trimmed = line.trimStart()

    if (trimmed.startsWith("```")) {
      inFence = !inFence
      continue
    }

    if (!inFence) {
      continue
    }

    if (line.length <= MAX_LINE_LENGTH) {
      continue
    }

    if (EXCLUDED_LINE_PATTERNS.some(pattern => line.includes(pattern))) {
      continue
    }

    violations.push({
      file,
      line: index + 1,
      length: line.length,
      content: line,
    })
  }

  return violations
}

function isExcluded(file: string): boolean {
  const normalized = file.split(path.sep).join(path.posix.sep)
  return EXCLUDED_PATH_SEGMENTS.some(segment => normalized.includes(segment))
}

function main() {
  const violations = ROOTS.flatMap(root => walk(root))
    .filter(file => !isExcluded(file))
    .flatMap(file => collectViolations(file))

  if (violations.length === 0) {
    return
  }

  console.error(
    `Found ${violations.length} code block line(s) longer than ${MAX_LINE_LENGTH} characters:\n`,
  )

  for (const violation of violations) {
    const relativePath = path
      .relative(process.cwd(), violation.file)
      .split(path.sep)
      .join(path.posix.sep)
    console.error(`${relativePath}:${violation.line} (${violation.length})`)
    console.error(`  ${violation.content}`)
  }

  process.exit(1)
}

main()
