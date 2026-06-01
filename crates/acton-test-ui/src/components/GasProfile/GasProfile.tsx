import type React from "react"
import {useEffect, useMemo, useRef, useState} from "react"
import flamegraph, {tooltip as flamegraphTooltip, type FlameGraphDatum} from "d3-flame-graph"
import {select} from "d3-selection"

import styles from "./GasProfile.module.css"

export interface GasProfileData {
  readonly total_gas: number
  readonly contracts: readonly GasProfileContract[]
}

export interface GasProfileReport extends GasProfileData {
  readonly tests?: readonly GasProfileTestReport[]
}

export interface GasProfileTestReport extends GasProfileData {
  readonly name: string
}

interface GasProfileContract {
  readonly name: string
  readonly total_gas: number
  readonly sample_count: number
  readonly samples: readonly GasProfileSample[]
}

interface GasProfileSample {
  readonly weight: number
  readonly frames: readonly GasProfileFrame[]
}

interface GasProfileFrame {
  readonly function_name: string
  readonly url: string
  readonly line_number: number
  readonly column_number: number
}

interface GasProfileProps {
  readonly profile: GasProfileData
  readonly projectRoot?: string
}

interface FlameNode {
  readonly id: string
  readonly name: string
  readonly url: string
  readonly lineNumber: number
  readonly columnNumber: number
  readonly selfGas: number
  readonly totalGas: number
  readonly children: FlameNode[]
}

type ConnectorLine = {
  readonly x1: number
  readonly y1: number
  readonly x2: number
  readonly y2: number
}

type FrameConnector = {
  readonly left: ConnectorLine
  readonly right: ConnectorLine
}

class MutableFlameNode {
  selfGas = 0
  totalGas = 0
  readonly children: MutableFlameNode[] = []
  readonly childrenByKey = new Map<string, MutableFlameNode>()

  constructor(
    readonly id: string,
    readonly name: string,
    readonly url = "",
    readonly lineNumber = -1,
    readonly columnNumber = -1,
  ) {}
}

const MIN_FLAME_WIDTH = 0.5
const ROW_HEIGHT = 22
const FRAME_SEPARATOR_CLASS = "gas-profile-frame-separator"
const FLAME_COLORS = [
  "#3d7ee8",
  "#2455dc",
  "#58b5d2",
  "#2f78df",
  "#285ed8",
  "#4ca2d9",
  "#356fe0",
  "#2b5bd1",
  "#63bfdb",
  "#3f85e6",
  "#214ccb",
  "#53acd4",
]

const numberFormatter = new Intl.NumberFormat("en-US")

const formatGas = (value: number) => numberFormatter.format(Math.round(value))

const formatPercent = (value: number, total: number) => {
  if (total <= 0) {
    return "0.0%"
  }

  return `${((value / total) * 100).toFixed(1)}%`
}

const getRelativePath = (filePath: string, projectRoot?: string) => {
  if (!filePath) {
    return "unknown source"
  }

  const normalizedProjectRoot = projectRoot?.replace(/\/+$/, "")
  if (
    normalizedProjectRoot &&
    (filePath === normalizedProjectRoot || filePath.startsWith(`${normalizedProjectRoot}/`))
  ) {
    return filePath.slice(normalizedProjectRoot.length + 1) || filePath
  }

  const pathSegments = filePath.split("/")
  if (pathSegments.length > 4) {
    return `.../${pathSegments.slice(-4).join("/")}`
  }

  return filePath
}

const formatLocation = (node: FlameNode, projectRoot?: string) => {
  if (!node.url) {
    return "unknown source"
  }

  const relativePath = getRelativePath(node.url, projectRoot)
  if (node.lineNumber < 0) {
    return relativePath
  }

  const line = node.lineNumber + 1
  const column = node.columnNumber >= 0 ? `:${node.columnNumber + 1}` : ""
  return `${relativePath}:${line}${column}`
}

const createMutableNode = (
  id: string,
  name: string,
  url = "",
  lineNumber = -1,
  columnNumber = -1,
): MutableFlameNode => new MutableFlameNode(id, name, url, lineNumber, columnNumber)

const frameKey = (frame: GasProfileFrame) =>
  `${frame.function_name}\u0000${frame.url}\u0000${frame.line_number}\u0000${frame.column_number}`

const freezeNode = (node: MutableFlameNode): FlameNode => {
  const children = node.children.map(child => freezeNode(child))
  children.sort((a, b) => b.totalGas - a.totalGas || a.name.localeCompare(b.name))
  node.totalGas = node.selfGas + children.reduce((total, child) => total + child.totalGas, 0)

  return {
    id: node.id,
    name: node.name,
    url: node.url,
    lineNumber: node.lineNumber,
    columnNumber: node.columnNumber,
    selfGas: node.selfGas,
    totalGas: node.totalGas,
    children,
  }
}

const toFlameGraphDatum = (node: FlameNode): FlameGraphDatum => ({
  id: node.id,
  name: node.name,
  value: node.totalGas,
  selfGas: node.selfGas,
  totalGas: node.totalGas,
  url: node.url,
  lineNumber: node.lineNumber,
  columnNumber: node.columnNumber,
  children: node.children.map(child => toFlameGraphDatum(child)),
})

const buildContractTree = (contract: GasProfileContract): FlameNode => {
  const root = createMutableNode(`contract:${contract.name}`, contract.name)

  for (const sample of contract.samples) {
    let current = root
    const pathParts = [root.id]

    for (const frame of sample.frames) {
      const key = frameKey(frame)
      pathParts.push(key)

      let child = current.childrenByKey.get(key)
      if (child === undefined) {
        child = createMutableNode(
          pathParts.join("/"),
          frame.function_name,
          frame.url,
          frame.line_number,
          frame.column_number,
        )
        current.childrenByKey.set(key, child)
        current.children.push(child)
      }

      current = child
    }

    current.selfGas += sample.weight
  }

  return freezeNode(root)
}

const colorForDepth = (depth: number) => {
  return FLAME_COLORS[depth % FLAME_COLORS.length]
}

const flameDatumString = (node: FlameGraphDatum, key: string) => {
  const value = node[key]
  return typeof value === "string" ? value : ""
}

const flameDatumNumber = (node: FlameGraphDatum, key: string) => {
  const value = node[key]
  return typeof value === "number" ? value : 0
}

const findFallbackSourceNode = (root: FlameNode): FlameNode | undefined => {
  let onInternalMessage: FlameNode | undefined
  let firstSourceNode: FlameNode | undefined

  const visit = (node: FlameNode) => {
    if (node.url) {
      firstSourceNode ??= node
      if (node.name === "onInternalMessage") {
        onInternalMessage ??= node
      }
    }

    for (const child of node.children) {
      visit(child)
    }
  }

  visit(root)
  return onInternalMessage ?? firstSourceNode
}

const getFrameDatum = (frame: SVGGElement): FlameGraphDatum | undefined => {
  return (frame as Readonly<{readonly __data__?: Readonly<{readonly data?: FlameGraphDatum}>}>)
    .__data__?.data
}

const updateFrameSeparators = (element: HTMLElement) => {
  const frames = element.querySelectorAll<SVGGElement>(".d3-flame-graph g.frame")

  for (const frame of frames) {
    const rect = frame.querySelector<SVGRectElement>("rect")
    if (rect === null) {
      continue
    }

    const width = rect.width.baseVal.value
    const height = rect.height.baseVal.value
    if (width <= 0 || height <= 0) {
      continue
    }

    let separator = frame.querySelector<SVGLineElement>(`.${FRAME_SEPARATOR_CLASS}`)
    if (separator === null) {
      separator = document.createElementNS("http://www.w3.org/2000/svg", "line")
      separator.classList.add(FRAME_SEPARATOR_CLASS)
      frame.append(separator)
    }

    const x = Math.max(0, width - 0.5)
    separator.setAttribute("x1", `${x}`)
    separator.setAttribute("x2", `${x}`)
    separator.setAttribute("y1", "0")
    separator.setAttribute("y2", `${height}`)
  }
}

const scheduleFrameSeparatorsUpdate = (element: HTMLElement) => {
  requestAnimationFrame(() => updateFrameSeparators(element))
}

const findRenderedFrameById = (
  element: HTMLElement,
  selectedFrameId: string,
): SVGGElement | undefined => {
  const frames = element.querySelectorAll<SVGGElement>(".d3-flame-graph g.frame")

  for (const frame of frames) {
    const frameDatum = getFrameDatum(frame)
    if (frameDatum !== undefined && flameDatumString(frameDatum, "id") === selectedFrameId) {
      return frame
    }
  }

  return undefined
}

const buildFrameConnector = (
  viewer: HTMLElement,
  flameChart: HTMLElement,
  details: HTMLElement,
  selectedFrameId: string,
): FrameConnector | undefined => {
  const selectedFrameElement = findRenderedFrameById(flameChart, selectedFrameId)
  const selectedRect = selectedFrameElement?.querySelector<SVGRectElement>("rect")
  if (selectedRect === undefined || selectedRect === null) {
    return undefined
  }

  const viewerRect = viewer.getBoundingClientRect()
  const frameRect = selectedRect.getBoundingClientRect()
  const detailsRect = details.getBoundingClientRect()
  if (frameRect.width <= 0 || frameRect.height <= 0 || detailsRect.width <= 0) {
    return undefined
  }

  const frameY = frameRect.bottom - viewerRect.top
  const detailsY = detailsRect.top - viewerRect.top

  return {
    left: {
      x1: detailsRect.left - viewerRect.left,
      y1: detailsY,
      x2: frameRect.left - viewerRect.left,
      y2: frameY,
    },
    right: {
      x1: detailsRect.right - viewerRect.left,
      y1: detailsY,
      x2: frameRect.right - viewerRect.left,
      y2: frameY,
    },
  }
}

export const GasProfile: React.FC<GasProfileProps> = ({profile, projectRoot}) => {
  const [selectedContractName, setSelectedContractName] = useState<string | undefined>(
    () => profile.contracts[0]?.name,
  )
  const viewerRef = useRef<HTMLElement | null>(null)
  const flameContainerRef = useRef<HTMLDivElement | null>(null)
  const flameChartRef = useRef<HTMLDivElement | null>(null)
  const detailsRef = useRef<HTMLDivElement | null>(null)
  const [flameWidth, setFlameWidth] = useState(0)
  const [selectedFrame, setSelectedFrame] = useState<FlameGraphDatum | undefined>()
  const [selectedStack, setSelectedStack] = useState<readonly string[]>([])
  const [frameConnector, setFrameConnector] = useState<FrameConnector | undefined>()

  const selectedContract = useMemo(() => {
    if (selectedContractName === undefined) {
      return profile.contracts[0]
    }

    return (
      profile.contracts.find(contract => contract.name === selectedContractName) ??
      profile.contracts[0]
    )
  }, [profile.contracts, selectedContractName])

  useEffect(() => {
    if (selectedContract === undefined) {
      setSelectedContractName(undefined)
      return
    }

    setSelectedContractName(selectedContract.name)
  }, [selectedContract])

  useEffect(() => {
    const element = flameContainerRef.current
    if (element === null) {
      return
    }

    const observer = new ResizeObserver(entries => {
      const entry = entries[0]
      if (entry !== undefined) {
        setFlameWidth(Math.max(0, Math.floor(entry.contentRect.width)))
      }
    })

    observer.observe(element)
    setFlameWidth(Math.max(0, Math.floor(element.getBoundingClientRect().width)))

    return () => observer.disconnect()
  }, [])

  const selectedTree = useMemo(
    () => selectedContract && buildContractTree(selectedContract),
    [selectedContract],
  )

  const flameData = useMemo(() => selectedTree && toFlameGraphDatum(selectedTree), [selectedTree])

  useEffect(() => {
    setSelectedFrame(undefined)
    setSelectedStack([])
  }, [selectedTree])

  useEffect(() => {
    const element = flameChartRef.current
    if (
      element === null ||
      flameData === undefined ||
      flameWidth <= 0 ||
      selectedContract === undefined
    ) {
      return
    }

    const chart = flamegraph()
      .width(flameWidth)
      .minHeight(220)
      .cellHeight(ROW_HEIGHT)
      .minFrameSize(MIN_FLAME_WIDTH)
      .transitionDuration(0)
      .inverted(true)
      .selfValue(false)
      .sort(
        (a, b) =>
          (b.value ?? 0) - (a.value ?? 0) ||
          flameDatumString(a.data, "name").localeCompare(flameDatumString(b.data, "name")),
      )
      .setColorMapper(node => colorForDepth(node.depth))
      .setLabelHandler(node => {
        const totalGas = node.value ?? flameDatumNumber(node.data, "totalGas")
        return `${flameDatumString(node.data, "name")} (${formatPercent(
          totalGas,
          selectedContract.total_gas,
        )}, ${formatGas(totalGas)} gas)`
      })
      .tooltip(
        flamegraphTooltip.defaultFlamegraphTooltip().text(node => {
          const totalGas = node.value ?? flameDatumNumber(node.data, "totalGas")
          return `${flameDatumString(node.data, "name")} · ${formatGas(totalGas)} gas · self ${formatGas(
            flameDatumNumber(node.data, "selfGas"),
          )} gas`
        }),
      )
      .onClick(node => {
        setSelectedFrame(node.data)
        setSelectedStack(
          [...node.ancestors()].reverse().map(ancestor => flameDatumString(ancestor.data, "name")),
        )
        scheduleFrameSeparatorsUpdate(element)
      })

    element.replaceChildren()
    select(element).datum(flameData).call(chart)
    scheduleFrameSeparatorsUpdate(element)

    return () => {
      chart.destroy()
      element.replaceChildren()
    }
  }, [flameData, flameWidth, selectedContract])

  const selectedNode = selectedFrame
  const selectedNodeValue =
    selectedNode === undefined ? 0 : flameDatumNumber(selectedNode, "totalGas")
  const selectedNodeSelfGas =
    selectedNode === undefined ? 0 : flameDatumNumber(selectedNode, "selfGas")
  const selectedNodeId = selectedNode === undefined ? "" : flameDatumString(selectedNode, "id")
  const selectedNodeName = selectedNode === undefined ? "" : flameDatumString(selectedNode, "name")
  const selectedNodeUrl = selectedNode === undefined ? "" : flameDatumString(selectedNode, "url")
  const selectedNodeSource =
    selectedNodeUrl === "" && selectedTree !== undefined
      ? findFallbackSourceNode(selectedTree)
      : undefined
  const selectedNodeSourceUrl = selectedNodeUrl || selectedNodeSource?.url || ""
  const selectedNodeLocation =
    selectedNode === undefined
      ? ""
      : formatLocation(
          {
            id: flameDatumString(selectedNode, "id"),
            name: selectedNodeName,
            url: selectedNodeSourceUrl,
            lineNumber:
              selectedNodeUrl || selectedNodeSource === undefined
                ? flameDatumNumber(selectedNode, "lineNumber")
                : selectedNodeSource.lineNumber,
            columnNumber:
              selectedNodeUrl || selectedNodeSource === undefined
                ? flameDatumNumber(selectedNode, "columnNumber")
                : selectedNodeSource.columnNumber,
            selfGas: selectedNodeSelfGas,
            totalGas: selectedNodeValue,
            children: [],
          },
          projectRoot,
        )

  const selectedNodeShare = selectedContract
    ? formatPercent(selectedNodeValue, selectedContract.total_gas)
    : "0.0%"

  useEffect(() => {
    const viewer = viewerRef.current
    const flameContainer = flameContainerRef.current
    const flameChartElement = flameChartRef.current
    const details = detailsRef.current

    if (
      viewer === null ||
      flameContainer === null ||
      flameChartElement === null ||
      details === null ||
      selectedNodeId === ""
    ) {
      setFrameConnector(undefined)
      return
    }

    let animationFrame = 0
    const scheduleConnectorUpdate = () => {
      if (animationFrame !== 0) {
        cancelAnimationFrame(animationFrame)
      }

      animationFrame = requestAnimationFrame(() => {
        animationFrame = 0
        setFrameConnector(buildFrameConnector(viewer, flameChartElement, details, selectedNodeId))
      })
    }

    const resizeObserver = new ResizeObserver(scheduleConnectorUpdate)
    resizeObserver.observe(viewer)
    resizeObserver.observe(flameContainer)
    resizeObserver.observe(details)
    window.addEventListener("resize", scheduleConnectorUpdate)
    flameContainer.addEventListener("scroll", scheduleConnectorUpdate, {passive: true})

    scheduleConnectorUpdate()

    return () => {
      if (animationFrame !== 0) {
        cancelAnimationFrame(animationFrame)
      }

      resizeObserver.disconnect()
      window.removeEventListener("resize", scheduleConnectorUpdate)
      flameContainer.removeEventListener("scroll", scheduleConnectorUpdate)
    }
  }, [flameWidth, selectedNodeId])

  if (profile.contracts.length === 0 || profile.total_gas === 0) {
    return <div className={styles.emptyState}>No gas profile samples were recorded.</div>
  }

  return (
    <div className={styles.gasProfile}>
      <div className={styles.contractSelector} aria-label="Contract gas profiles">
        {profile.contracts.map(contract => {
          const isSelected = contract.name === selectedContract?.name

          return (
            <button
              key={contract.name}
              type="button"
              className={`${styles.contractButton} ${isSelected ? styles.contractButtonSelected : ""}`}
              title={contract.name}
              onClick={() => setSelectedContractName(contract.name)}
            >
              {contract.name}
            </button>
          )
        })}
      </div>

      <div className={styles.workspace}>
        <section className={styles.viewer} ref={viewerRef}>
          {selectedContract === undefined || selectedTree === undefined ? (
            <div className={styles.emptyState}>Select a contract to inspect its gas profile.</div>
          ) : (
            <>
              <div className={styles.flameWrap} ref={flameContainerRef}>
                {flameWidth <= 0 || flameData === undefined ? (
                  <div className={styles.viewerState}>Preparing flamegraph...</div>
                ) : (
                  <div
                    ref={flameChartRef}
                    className={styles.flameChart}
                    role="img"
                    aria-label={`Gas flamegraph for ${selectedContract.name}`}
                  />
                )}
              </div>

              {selectedNode !== undefined && (
                <>
                  {frameConnector !== undefined && (
                    <svg className={styles.connectorLayer} aria-hidden="true">
                      <line
                        x1={frameConnector.left.x1}
                        y1={frameConnector.left.y1}
                        x2={frameConnector.left.x2}
                        y2={frameConnector.left.y2}
                      />
                      <line
                        x1={frameConnector.right.x1}
                        y1={frameConnector.right.y1}
                        x2={frameConnector.right.x2}
                        y2={frameConnector.right.y2}
                      />
                    </svg>
                  )}
                  <div className={styles.details} ref={detailsRef}>
                    <nav className={styles.stackPath} aria-label="Call stack">
                      {selectedStack.map((part, index) => (
                        <span key={`${part}-${index}`} className={styles.stackPart}>
                          {part}
                        </span>
                      ))}
                    </nav>
                    <div className={styles.detailsHeader}>
                      <div className={styles.detailsTitle} title={selectedNodeName}>
                        {selectedNodeName}
                      </div>
                      <div className={styles.detailsLocation} title={selectedNodeSourceUrl}>
                        {selectedNodeLocation}
                      </div>
                    </div>
                    <div className={styles.metricGrid}>
                      <div className={styles.metric}>
                        <span>Total</span>
                        <strong>{formatGas(selectedNodeValue)}</strong>
                      </div>
                      <div className={styles.metric}>
                        <span>Self</span>
                        <strong>{formatGas(selectedNodeSelfGas)}</strong>
                      </div>
                      <div className={styles.metric}>
                        <span>Contract Share</span>
                        <strong>{selectedNodeShare}</strong>
                      </div>
                    </div>
                  </div>
                </>
              )}
            </>
          )}
        </section>
      </div>
    </div>
  )
}
