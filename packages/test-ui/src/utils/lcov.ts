export type CoverageLineStatus = "covered" | "partial" | "uncovered"

export interface CoverageLine {
  readonly lineNumber: number
  readonly hits: number
  readonly branchesFound: number
  readonly branchesHit: number
  readonly status: CoverageLineStatus
}

export interface CoverageFile {
  readonly filePath: string
  readonly linesFound: number
  readonly linesHit: number
  readonly branchesFound: number
  readonly branchesHit: number
  readonly combinedScore: number
  readonly linePercentage: number
  readonly branchPercentage: number | undefined
  readonly firstUncoveredLine: number | undefined
  readonly firstPartialLine: number | undefined
  readonly lines: ReadonlyMap<number, CoverageLine>
}

export interface CoverageSummary {
  readonly files: readonly CoverageFile[]
  readonly totalFiles: number
  readonly totalLinesFound: number
  readonly totalLinesHit: number
  readonly totalBranchesFound: number
  readonly totalBranchesHit: number
  readonly combinedScore: number
  readonly linePercentage: number
  readonly branchPercentage: number | undefined
}

interface MutableCoverageFile {
  readonly filePath: string
  readonly lineHits: Map<number, number>
  readonly branchHits: Map<number, readonly number[]>
  readonly stats: {
    linesFound: number | undefined
    linesHit: number | undefined
    branchesFound: number | undefined
    branchesHit: number | undefined
  }
}

const parseInteger = (value: string): number | undefined => {
  const parsed = Number.parseInt(value, 10)
  return Number.isNaN(parsed) ? undefined : parsed
}

const clampPercentage = (covered: number, total: number): number | undefined => {
  if (total <= 0) {
    return undefined
  }

  return (covered / total) * 100
}

const finalizeFile = (file: MutableCoverageFile): CoverageFile => {
  const lines = new Map<number, CoverageLine>()
  const sortedLineNumbers = [...file.lineHits.keys()].sort((left, right) => left - right)
  let computedLinesHit = 0
  let computedBranchesFound = 0
  let computedBranchesHit = 0
  let firstUncoveredLine: number | undefined
  let firstPartialLine: number | undefined

  for (const lineNumber of sortedLineNumbers) {
    const hits = file.lineHits.get(lineNumber) ?? 0
    const branchHits = file.branchHits.get(lineNumber) ?? []
    const branchesFound = branchHits.length
    const branchesHit = branchHits.filter(branchHit => branchHit > 0).length
    const status: CoverageLineStatus =
      hits === 0
        ? "uncovered"
        : branchesFound > 0 && branchesHit < branchesFound
          ? "partial"
          : "covered"

    if (hits > 0) {
      computedLinesHit += 1
    }

    computedBranchesFound += branchesFound
    computedBranchesHit += branchesHit

    if (status === "uncovered" && firstUncoveredLine === undefined) {
      firstUncoveredLine = lineNumber
    }

    if (status === "partial" && firstPartialLine === undefined) {
      firstPartialLine = lineNumber
    }

    lines.set(lineNumber, {
      lineNumber,
      hits,
      branchesFound,
      branchesHit,
      status,
    })
  }

  const linesFound = file.stats.linesFound ?? lines.size
  const linesHit = file.stats.linesHit ?? computedLinesHit
  const branchesFound = file.stats.branchesFound ?? computedBranchesFound
  const branchesHit = file.stats.branchesHit ?? computedBranchesHit
  const linePercentage = clampPercentage(linesHit, linesFound) ?? 0
  const combinedScore = clampPercentage(linesHit + branchesHit, linesFound + branchesFound) ?? 0

  return {
    filePath: file.filePath,
    linesFound,
    linesHit,
    branchesFound,
    branchesHit,
    combinedScore,
    linePercentage,
    branchPercentage: clampPercentage(branchesHit, branchesFound),
    firstUncoveredLine,
    firstPartialLine,
    lines,
  }
}

export const parseLcov = (content: string): CoverageSummary => {
  const files: CoverageFile[] = []
  let currentFile: MutableCoverageFile | undefined

  const pushCurrentFile = () => {
    if (currentFile === undefined) {
      return
    }

    files.push(finalizeFile(currentFile))
    currentFile = undefined
  }

  for (const rawLine of content.split(/\r?\n/u)) {
    const line = rawLine.trim()
    if (line.length === 0) {
      continue
    }

    if (line === "end_of_record") {
      pushCurrentFile()
      continue
    }

    const separatorIndex = line.indexOf(":")
    if (separatorIndex === -1) {
      continue
    }

    const recordType = line.slice(0, separatorIndex)
    const payload = line.slice(separatorIndex + 1)

    if (recordType === "SF") {
      pushCurrentFile()
      currentFile = {
        filePath: payload,
        lineHits: new Map<number, number>(),
        branchHits: new Map<number, readonly number[]>(),
        stats: {
          linesFound: undefined,
          linesHit: undefined,
          branchesFound: undefined,
          branchesHit: undefined,
        },
      }
      continue
    }

    if (currentFile === undefined) {
      continue
    }

    if (recordType === "DA") {
      const [lineNumberRaw, hitsRaw] = payload.split(",", 2)
      const lineNumber = parseInteger(lineNumberRaw)
      const hits = parseInteger(hitsRaw)
      if (lineNumber !== undefined && hits !== undefined) {
        currentFile.lineHits.set(lineNumber, (currentFile.lineHits.get(lineNumber) ?? 0) + hits)
      }
      continue
    }

    if (recordType === "LF") {
      currentFile.stats.linesFound = parseInteger(payload)
      continue
    }

    if (recordType === "LH") {
      currentFile.stats.linesHit = parseInteger(payload)
      continue
    }

    if (recordType === "BRF") {
      currentFile.stats.branchesFound = parseInteger(payload)
      continue
    }

    if (recordType === "BRH") {
      currentFile.stats.branchesHit = parseInteger(payload)
      continue
    }

    if (recordType !== "BRDA") {
      continue
    }

    const [lineNumberRaw, _blockNumberRaw, _branchNumberRaw, takenRaw] = payload.split(",", 4)
    const lineNumber = parseInteger(lineNumberRaw)
    if (lineNumber === undefined) {
      continue
    }

    const taken = takenRaw === "-" ? 0 : parseInteger(takenRaw)
    if (taken === undefined) {
      continue
    }

    const existingHits = currentFile.branchHits.get(lineNumber) ?? []
    currentFile.branchHits.set(lineNumber, [...existingHits, taken])
  }

  pushCurrentFile()

  files.sort((left, right) => {
    if (left.combinedScore !== right.combinedScore) {
      return left.combinedScore - right.combinedScore
    }

    if (left.linesFound !== right.linesFound) {
      return right.linesFound - left.linesFound
    }

    return left.filePath.localeCompare(right.filePath)
  })

  const totalLinesFound = files.reduce((total, file) => total + file.linesFound, 0)
  const totalLinesHit = files.reduce((total, file) => total + file.linesHit, 0)
  const totalBranchesFound = files.reduce((total, file) => total + file.branchesFound, 0)
  const totalBranchesHit = files.reduce((total, file) => total + file.branchesHit, 0)
  const combinedScore =
    clampPercentage(totalLinesHit + totalBranchesHit, totalLinesFound + totalBranchesFound) ?? 0

  return {
    files,
    totalFiles: files.length,
    totalLinesFound,
    totalLinesHit,
    totalBranchesFound,
    totalBranchesHit,
    combinedScore,
    linePercentage: clampPercentage(totalLinesHit, totalLinesFound) ?? 0,
    branchPercentage: clampPercentage(totalBranchesHit, totalBranchesFound),
  }
}
