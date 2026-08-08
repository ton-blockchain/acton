import {Boxes, FlaskConical, LayoutDashboard} from "lucide-react"
import type {LucideIcon} from "lucide-react"

export type StudioPath = "/" | "/virtual-environments" | "/tests"

export interface StudioPage {
  readonly path: StudioPath
  readonly label: string
  readonly shortDescription: string
  readonly icon: LucideIcon
}

export interface StudioFeaturePage extends StudioPage {
  readonly actionLabel: string
  readonly emptyTitle: string
  readonly emptyDescription: string
}

export const studioPages: readonly StudioPage[] = [
  {
    path: "/",
    label: "Overview",
    shortDescription: "Manage localnets, simulations and test activity in one workspace",
    icon: LayoutDashboard,
  },
  {
    path: "/tests",
    label: "Tests",
    shortDescription: "Run Acton tests and inspect failures, traces, gas data and history",
    icon: FlaskConical,
  },
  {
    path: "/virtual-environments",
    label: "Virtual Environments",
    shortDescription:
      "Create isolated TON networks, keep presets and move between active environments",
    icon: Boxes,
  },
]

export const studioFeaturePages: Readonly<Record<Exclude<StudioPath, "/">, StudioFeaturePage>> = {
  "/virtual-environments": {
    ...studioPages[2],
    actionLabel: "Create environment",
    emptyTitle: "No virtual environments yet",
    emptyDescription:
      "Your environments will appear here after Studio is connected to the Acton runtime",
  },
  "/tests": {
    ...studioPages[1],
    actionLabel: "Run tests",
    emptyTitle: "No test runs",
    emptyDescription:
      "Runs started from Studio or the CLI will appear here through the same reporter pipeline",
  },
}

export function isStudioPath(pathname: string): pathname is StudioPath {
  return studioPages.some(page => page.path === pathname)
}
