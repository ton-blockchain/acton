import type {Step} from "ton-assembly/dist/trace"

/**
 * Chooses the closest 1-based line number for a step that may map to multiple source lines.
 * - If prevStepLine is undefined, uses primary line (loc.line + 1)
 * - Otherwise, selects candidate from [loc.line, ...otherLines] that minimizes |candidate+1 - prevStepLine|
 * - On ties, prefers the primary line; then the smallest distance greater-or-equal to prev (to reduce back jumps)
 */
export function selectClosestLine(
  prevStepLine: number | undefined,
  nextStep: Step,
): number | undefined {
  const loc = nextStep.loc
  if (!loc) return undefined

  const candidates = [loc.line, ...loc.otherLines]
    .map(it => it + 1) // convert to 1-based
    .filter(it => it > 0)

  if (prevStepLine === undefined || candidates.length === 0) {
    return loc.line + 1
  }

  let best = candidates[0]
  let bestDist = Math.abs(best - prevStepLine)

  const primary = loc.line + 1

  for (const c of candidates) {
    const dist = Math.abs(c - prevStepLine)
    if (dist < bestDist) {
      best = c
      bestDist = dist
    } else if (dist === bestDist) {
      // prefer primary
      if (best !== primary && c === primary) {
        best = c
      } else if (best !== primary && c !== primary) {
        // prefer non-backward when possible
        const bestBackward = best < prevStepLine
        const cBackward = c < prevStepLine
        if (bestBackward && !cBackward) best = c
      }
    }
  }

  return best
}
