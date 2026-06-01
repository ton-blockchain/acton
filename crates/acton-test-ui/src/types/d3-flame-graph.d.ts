declare module "d3-flame-graph" {
  export interface FlameGraphDatum {
    name: string
    value: number
    children?: FlameGraphDatum[]
    [key: string]: unknown
  }

  export interface FlameGraphNode {
    data: FlameGraphDatum
    depth: number
    value?: number
    x0: number
    x1: number
    ancestors(): FlameGraphNode[]
  }

  export interface FlameGraphChart {
    (selection: any): void
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
  export function select(node: Element): any
}
