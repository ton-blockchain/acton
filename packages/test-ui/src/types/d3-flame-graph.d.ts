declare module "d3-flame-graph" {
  export interface FlameGraphDatum {
    readonly name: string
    readonly value: number
    readonly children?: readonly FlameGraphDatum[]
    readonly [key: string]: unknown
  }

  export interface FlameGraphNode {
    readonly data: FlameGraphDatum
    readonly depth: number
    readonly value?: number
    readonly x0: number
    readonly x1: number
    ancestors(): readonly FlameGraphNode[]
  }

  export interface FlameGraphChart {
    (selection: import("d3-selection").Selection<FlameGraphDatum>): void
    width(value: number): this
    minHeight(value: number): this
    cellHeight(value: number): this
    minFrameSize(value: number): this
    transitionDuration(value: number): this
    inverted(value: boolean): this
    selfValue(value: boolean): this
    sort(value: boolean | ((a: FlameGraphNode, b: FlameGraphNode) => number)): this
    setLabelHandler(value: (node: FlameGraphNode) => string): this
    setColorMapper(value: (node: FlameGraphNode, originalColor: string) => string): this
    onClick(value: (node: FlameGraphNode) => void): this
    tooltip(value: unknown): this
    destroy(): this
    update(data?: FlameGraphDatum): this
  }

  export interface FlameGraphTooltip {
    text(value: (node: FlameGraphNode) => string): this
    destroy?(): void
  }

  export const tooltip: {
    defaultFlamegraphTooltip(): FlameGraphTooltip
  }

  export default function flamegraph(): FlameGraphChart
}

declare module "d3-selection" {
  export interface Selection<Datum = unknown> {
    readonly datum: <NextDatum>(value: NextDatum) => Selection<NextDatum>
    readonly call: (callback: (selection: Selection<Datum>) => void) => Selection<Datum>
  }

  export function select(node: Element): Selection
}
