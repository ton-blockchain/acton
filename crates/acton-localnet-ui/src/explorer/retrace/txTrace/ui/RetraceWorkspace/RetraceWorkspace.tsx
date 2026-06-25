import {
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  lazy,
  memo,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"

import type {StackElement} from "ton-assembly/dist/trace"
import {
  Braces,
  Bug,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  FileCode2,
  ListTree,
  SkipBack,
  SkipForward,
} from "lucide-react"

import {Tooltip} from "@acton/shared-ui"

import type {
  SourceBundle,
  SourceFile,
  SourceTraceLocation,
  SourceTraceResponse,
  SourceTraceStep,
  SourceTraceVariable,
} from "../../../../api/types"
import {useLineExecutionData, useTraceStepper} from "../../hooks"
import type {RetraceResultAndCode} from "../../lib/types"
import {
  buildInstructionDetails,
  calculateCumulativeGasSinceBegin,
  getImplicitRet,
  getStoredSourceDebugPanelWidth,
  getStoredTraceViewMode,
  setStoredSourceDebugPanelWidth,
  setStoredTraceViewMode,
  type TraceViewMode,
} from "../../lib/traceViewModel"
import InlineLoader from "../InlineLoader"
import StatusBadge from "../StatusBadge"
import TraceSidePanel from "../TraceSidePanel"
import TraceStepsChainView from "../TraceStepsChainView"
import TraceViewModeToggle from "../TraceViewModeToggle"
import StackItemDetails from "../stack/StackItemDetails"

import styles from "./RetraceWorkspace.module.css"

const CodeEditor = lazy(() => import("../../../ui/CodeEditor"))

type WorkspaceTab = "trace" | "sources"
type SourceEditorLanguage = "tolk" | "func" | "tasm"
type SourceDebugSectionId = "exception" | "locals" | "callStack"

const SOURCE_DEBUG_SECTION_MIN_HEIGHT = 72
const SOURCE_DEBUG_PANEL_DEFAULT_WIDTH = 360
const SOURCE_DEBUG_PANEL_MIN_WIDTH = 280
const SOURCE_DEBUG_PANEL_MAX_WIDTH = 720
const SOURCE_DEBUG_EDITOR_MIN_WIDTH = 360

const DEFAULT_SOURCE_DEBUG_SECTION_HEIGHTS: Record<SourceDebugSectionId, number> = {
  exception: 180,
  locals: 180,
  callStack: 180,
}

const DEFAULT_SOURCE_DEBUG_COLLAPSED: Record<SourceDebugSectionId, boolean> = {
  exception: false,
  locals: false,
  callStack: false,
}

interface RetraceWorkspaceProps {
  readonly result: RetraceResultAndCode
  readonly className?: string
}

function normalizeSourcePath(path: string): string {
  return path.replace(/\\/g, "/").replace(/^\.\//, "")
}

function findSourceFile(files: readonly SourceFile[], path: string): SourceFile | undefined {
  const normalizedPath = normalizeSourcePath(path)
  return files.find(file => normalizeSourcePath(file.path) === normalizedPath)
}

function sourcePathBasename(path: string): string {
  const normalizedPath = normalizeSourcePath(path)
  return normalizedPath.split("/").pop() || normalizedPath
}

function defaultSourcePath(bundle: SourceBundle): string {
  return findSourceFile(bundle.files, bundle.entrypoint)?.path ?? bundle.files[0]?.path ?? ""
}

function shouldShowSourceFile(path: string): boolean {
  return !normalizeSourcePath(path).toLowerCase().endsWith(".abi.json")
}

function visibleSourceBundle(bundle: SourceBundle): SourceBundle {
  return {
    ...bundle,
    sources: bundle.sources.filter(source => shouldShowSourceFile(source.path)),
    files: bundle.files.filter(file => shouldShowSourceFile(file.path)),
  }
}

function sourceFileLabel(path: string): string {
  return sourcePathBasename(path)
}

function sourceLocationLabel(location: SourceTraceLocation): string {
  return `${sourceFileLabel(location.file)}:${location.line}`
}

function sourceLocationWithColumnLabel(location: SourceTraceLocation): string {
  return `${sourceFileLabel(location.file)}:${location.line}:${location.column}`
}

function decodeSourceFile(file: SourceFile): string {
  if (file.content_text !== null) {
    return file.content_text
  }

  try {
    const binary = globalThis.atob(file.content_base64)
    const bytes = Uint8Array.from(binary, char => char.charCodeAt(0))
    return new TextDecoder().decode(bytes)
  } catch {
    return ""
  }
}

function sourceLanguage(path: string): SourceEditorLanguage {
  const normalizedPath = normalizeSourcePath(path).toLowerCase()
  if (normalizedPath.endsWith(".fc") || normalizedPath.endsWith(".func")) {
    return "func"
  }
  if (normalizedPath.endsWith(".tasm") || normalizedPath.endsWith(".asm")) {
    return "tasm"
  }
  return "tolk"
}

function shouldIgnoreSourceDebugKey(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false
  }

  return Boolean(target.closest("input, textarea, select, button, [contenteditable='true']"))
}

function clampSourceDebugPanelWidth(width: number, layoutWidth: number): number {
  const maxWidth = Math.max(
    SOURCE_DEBUG_PANEL_MIN_WIDTH,
    Math.min(SOURCE_DEBUG_PANEL_MAX_WIDTH, layoutWidth - SOURCE_DEBUG_EDITOR_MIN_WIDTH),
  )
  return Math.min(maxWidth, Math.max(SOURCE_DEBUG_PANEL_MIN_WIDTH, width))
}

function SourceDebugToolbar({
  selectedStep,
  totalSteps,
  currentStep,
  truncated,
  onFirst,
  onPrev,
  onNext,
  onLast,
}: {
  readonly selectedStep: number
  readonly totalSteps: number
  readonly currentStep?: SourceTraceStep
  readonly truncated: boolean
  readonly onFirst: () => void
  readonly onPrev: () => void
  readonly onNext: () => void
  readonly onLast: () => void
}) {
  const canGoPrev = totalSteps > 0 && selectedStep > 0
  const canGoNext = totalSteps > 0 && selectedStep < totalSteps - 1

  return (
    <div className={styles.sourceDebugToolbar} aria-label="Source trace controls">
      <div className={styles.sourceDebugNavigation}>
        <button
          type="button"
          className={styles.sourceDebugIconButton}
          onClick={onFirst}
          disabled={!canGoPrev}
          title="Go to first source step"
          aria-label="Go to first source step"
        >
          <SkipBack size={15} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={styles.sourceDebugIconButton}
          onClick={onPrev}
          disabled={!canGoPrev}
          title="Previous source step"
          aria-label="Previous source step"
        >
          <ChevronLeft size={16} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={styles.sourceDebugIconButton}
          onClick={onNext}
          disabled={!canGoNext}
          title="Next source step"
          aria-label="Next source step"
        >
          <ChevronRight size={16} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={styles.sourceDebugIconButton}
          onClick={onLast}
          disabled={!canGoNext}
          title="Go to last source step"
          aria-label="Go to last source step"
        >
          <SkipForward size={15} aria-hidden="true" />
        </button>
      </div>

      <div className={styles.sourceDebugStepInfo}>
        <span className={styles.sourceDebugStepCounter}>
          {totalSteps > 0 ? `Step ${selectedStep + 1} of ${totalSteps}` : "No source steps"}
        </span>
        {currentStep && (
          <>
            <span className={styles.sourceDebugLocation}>
              {`at ${sourceLocationLabel(currentStep.location)}`}
            </span>
          </>
        )}
        {truncated && <span className={styles.sourceDebugWarning}>truncated</span>}
      </div>
    </div>
  )
}

function SourceVariableList({
  variables,
  depth = 0,
  parentPath = "",
}: {
  readonly variables: readonly SourceTraceVariable[]
  readonly depth?: number
  readonly parentPath?: string
}) {
  if (variables.length === 0) {
    return <div className={styles.sourceDebugEmpty}>No locals</div>
  }

  return (
    <div className={styles.sourceVariableList} role={depth === 0 ? "tree" : "group"}>
      {variables.map((variable, index) => {
        const variablePath = `${parentPath}/${index}:${variable.name}`
        return (
          <SourceVariableRow
            key={variablePath}
            variable={variable}
            depth={depth}
            path={variablePath}
          />
        )
      })}
    </div>
  )
}

function SourceVariableIcon() {
  return (
    <svg
      className={styles.sourceVariableIcon}
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <path
        d="M3.5 14V12C3.5 11.7239 3.72386 11.5 4 11.5H12C12.2761 11.5 12.5 11.7239 12.5 12V14C12.5 14.2761 12.2761 14.5 12 14.5H4C3.72386 14.5 3.5 14.2761 3.5 14Z"
        stroke="currentColor"
      />
      <path
        d="M3.5 9V7C3.5 6.72386 3.72386 6.5 4 6.5H12C12.2761 6.5 12.5 6.72386 12.5 7V9C12.5 9.27614 12.2761 9.5 12 9.5H4C3.72386 9.5 3.5 9.27614 3.5 9Z"
        stroke="currentColor"
      />
      <path
        d="M3.5 4V2C3.5 1.72386 3.72386 1.5 4 1.5H12C12.2761 1.5 12.5 1.72386 12.5 2V4C12.5 4.27614 12.2761 4.5 12 4.5H4C3.72386 4.5 3.5 4.27614 3.5 4Z"
        stroke="currentColor"
      />
    </svg>
  )
}

function sourceVariableTooltip(variable: SourceTraceVariable): string {
  const typeLabel = variable.type ? ` {${variable.type}}` : ""
  return `${variable.name}${typeLabel} = ${variable.value}`
}

function SourceVariableRow({
  variable,
  depth,
  path,
}: {
  readonly variable: SourceTraceVariable
  readonly depth: number
  readonly path: string
}) {
  const hasChildren = variable.children.length > 0
  const [expanded, setExpanded] = useState(false)
  const rowStyle = {
    "--source-variable-depth": depth,
  } as CSSProperties
  const toggleExpanded = useCallback(() => {
    if (hasChildren) {
      setExpanded(current => !current)
    }
  }, [hasChildren])

  return (
    <div
      className={styles.sourceVariableGroup}
      role="treeitem"
      aria-expanded={hasChildren ? expanded : undefined}
    >
      <div
        className={styles.sourceVariableRow}
        style={rowStyle}
        onMouseDown={event => {
          if (
            event.detail > 1 &&
            !(event.target instanceof HTMLElement && event.target.closest("button"))
          ) {
            event.preventDefault()
          }
        }}
        onDoubleClick={event => {
          if (!(event.target instanceof HTMLElement && event.target.closest("button"))) {
            event.preventDefault()
            globalThis.getSelection()?.removeAllRanges()
            toggleExpanded()
          }
        }}
      >
        <button
          type="button"
          className={styles.sourceVariableToggle}
          onClick={toggleExpanded}
          disabled={!hasChildren}
          aria-label={`${expanded ? "Collapse" : "Expand"} ${variable.name}`}
        >
          {hasChildren ? (
            expanded ? (
              <ChevronDown size={14} aria-hidden="true" />
            ) : (
              <ChevronRight size={14} aria-hidden="true" />
            )
          ) : null}
        </button>
        <SourceVariableIcon />
        <span className={styles.sourceVariableExpression} title={sourceVariableTooltip(variable)}>
          <span className={styles.sourceVariableName}>{variable.name}</span>
          <span className={styles.sourceVariableEquals}> = </span>
          {variable.type && (
            <span className={styles.sourceVariableType}>{`{${variable.type}} `}</span>
          )}
          <span className={styles.sourceVariableValue}>{variable.value}</span>
        </span>
      </div>
      {hasChildren && expanded && (
        <SourceVariableList variables={variable.children} depth={depth + 1} parentPath={path} />
      )}
    </div>
  )
}

function callFrameLabel(frame: SourceTraceStep["call_stack"][number]): string {
  if (frame.is_builtin) {
    return `${frame.function_name} (built-in)`
  }
  if (frame.is_inlined) {
    return `${frame.function_name} (inlined)`
  }
  return frame.function_name
}

function SourceCallFrameIcon() {
  return (
    <svg
      className={styles.sourceCallFrameIcon}
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <circle cx="8" cy="8" r="6.5" fill="transparent" stroke="#DB6068" />
      <path
        d="M7.25726 12H8.2744V7.32741H9.82869V6.43027H8.2744V5.48C8.2744 5.12 8.52012 4.87429 8.8744 4.87429H9.88012V4H8.77726C7.8744 4 7.25726 4.57143 7.25726 5.40571V6.43027H6.12012V7.32741H7.25726V12Z"
        fill="#DB6068"
      />
    </svg>
  )
}

function SourceDebugResizableSection({
  id,
  title,
  icon,
  collapsed,
  height,
  grow,
  resizePartnerId,
  canResize,
  onToggle,
  onResizeStart,
  setSectionElement,
  children,
}: {
  readonly id: SourceDebugSectionId
  readonly title: string
  readonly icon?: ReactNode
  readonly collapsed: boolean
  readonly height: number
  readonly grow: boolean
  readonly resizePartnerId?: SourceDebugSectionId
  readonly canResize: boolean
  readonly onToggle: (id: SourceDebugSectionId) => void
  readonly onResizeStart: (
    id: SourceDebugSectionId,
    partnerId: SourceDebugSectionId,
    event: ReactPointerEvent<HTMLDivElement>,
  ) => void
  readonly setSectionElement: (id: SourceDebugSectionId, element: HTMLElement | null) => void
  readonly children: ReactNode
}) {
  const contentId = `source-debug-section-${id}`
  const style = {
    "--source-debug-section-height": `${height}px`,
  } as CSSProperties

  return (
    <section
      ref={element => setSectionElement(id, element)}
      className={`${styles.sourceDebugSectionPanel} ${
        collapsed ? styles.sourceDebugSectionPanelCollapsed : ""
      } ${grow ? styles.sourceDebugSectionPanelGrow : ""}`}
      style={collapsed ? undefined : style}
    >
      <button
        type="button"
        className={styles.sourceDebugSectionHeader}
        onClick={() => onToggle(id)}
        aria-expanded={!collapsed}
        aria-controls={contentId}
      >
        <span className={styles.sourceDebugSectionHeaderContent}>
          {collapsed ? (
            <ChevronRight size={14} aria-hidden="true" />
          ) : (
            <ChevronDown size={14} aria-hidden="true" />
          )}
          {icon}
          <span className={styles.sourceDebugSectionTitle}>{title}</span>
        </span>
      </button>
      {!collapsed && (
        <div id={contentId} className={styles.sourceDebugSectionBody}>
          {children}
        </div>
      )}
      {!collapsed && canResize && (
        <div
          className={styles.sourceDebugResizeHandle}
          role="separator"
          aria-label={`Resize ${title}`}
          aria-orientation="horizontal"
          aria-valuemin={SOURCE_DEBUG_SECTION_MIN_HEIGHT}
          aria-valuenow={Math.round(height)}
          onPointerDown={event => resizePartnerId && onResizeStart(id, resizePartnerId, event)}
        />
      )}
    </section>
  )
}

function SourceDebugPanel({
  step,
  selectedCallFrameIndex,
  onCallFrameSelect,
}: {
  readonly step?: SourceTraceStep
  readonly selectedCallFrameIndex: number | null
  readonly onCallFrameSelect: (index: number) => void
}) {
  const [collapsedSections, setCollapsedSections] = useState(DEFAULT_SOURCE_DEBUG_COLLAPSED)
  const [sectionHeights, setSectionHeights] = useState(DEFAULT_SOURCE_DEBUG_SECTION_HEIGHTS)
  const sectionElementsRef = useRef<Partial<Record<SourceDebugSectionId, HTMLElement | null>>>({})

  const setSectionElement = useCallback(
    (id: SourceDebugSectionId, element: HTMLElement | null) => {
      sectionElementsRef.current[id] = element
    },
    [],
  )

  const toggleSection = useCallback((id: SourceDebugSectionId) => {
    setCollapsedSections(current => ({
      ...current,
      [id]: !current[id],
    }))
  }, [])

  const startSectionResize = useCallback(
    (
      id: SourceDebugSectionId,
      partnerId: SourceDebugSectionId,
      event: ReactPointerEvent<HTMLDivElement>,
    ) => {
      event.preventDefault()

      const targetElement = sectionElementsRef.current[id]
      const partnerElement = sectionElementsRef.current[partnerId]
      if (!targetElement || !partnerElement) {
        return
      }

      const startY = event.clientY
      const measuredHeights = {...sectionHeights}
      for (const [sectionId, element] of Object.entries(sectionElementsRef.current) as [
        SourceDebugSectionId,
        HTMLElement | null,
      ][]) {
        if (element && !collapsedSections[sectionId]) {
          measuredHeights[sectionId] = Math.round(element.getBoundingClientRect().height)
        }
      }

      const startHeight = Math.max(
        SOURCE_DEBUG_SECTION_MIN_HEIGHT,
        Math.round(targetElement.getBoundingClientRect().height),
      )
      const partnerStartHeight = Math.max(
        SOURCE_DEBUG_SECTION_MIN_HEIGHT,
        Math.round(partnerElement.getBoundingClientRect().height),
      )
      const pairedHeight = startHeight + partnerStartHeight
      const maxHeight = pairedHeight - SOURCE_DEBUG_SECTION_MIN_HEIGHT
      const previousCursor = document.body.style.cursor
      const previousUserSelect = document.body.style.userSelect
      document.body.style.cursor = "row-resize"
      document.body.style.userSelect = "none"
      setSectionHeights(measuredHeights)

      const handlePointerMove = (moveEvent: globalThis.PointerEvent) => {
        const nextHeight = Math.min(
          maxHeight,
          Math.max(
            SOURCE_DEBUG_SECTION_MIN_HEIGHT,
            Math.round(startHeight + moveEvent.clientY - startY),
          ),
        )
        const nextPartnerHeight = Math.max(
          SOURCE_DEBUG_SECTION_MIN_HEIGHT,
          pairedHeight - nextHeight,
        )
        setSectionHeights(current =>
          current[id] === nextHeight && current[partnerId] === nextPartnerHeight
            ? current
            : {
                ...current,
                [id]: nextHeight,
                [partnerId]: nextPartnerHeight,
              },
        )
      }

      const finishResize = () => {
        document.body.style.cursor = previousCursor
        document.body.style.userSelect = previousUserSelect
        window.removeEventListener("pointermove", handlePointerMove)
        window.removeEventListener("pointerup", finishResize)
        window.removeEventListener("pointercancel", finishResize)
      }

      window.addEventListener("pointermove", handlePointerMove)
      window.addEventListener("pointerup", finishResize)
      window.addEventListener("pointercancel", finishResize)
    },
    [collapsedSections, sectionHeights],
  )

  if (!step) {
    return (
      <aside className={styles.sourceDebugPanel} aria-label="Source trace state">
        <div className={styles.sourceDebugEmptyState}>No source trace steps</div>
      </aside>
    )
  }

  const visibleSectionIds: SourceDebugSectionId[] = [
    ...(step.exception ? (["exception"] as const) : []),
    "locals",
    "callStack",
  ]
  const activeCallFrameIndex = selectedCallFrameIndex ?? (step.call_stack.length > 0 ? 0 : null)
  const resizePartnerId = (id: SourceDebugSectionId): SourceDebugSectionId | undefined => {
    const startIndex = visibleSectionIds.indexOf(id)
    if (startIndex < 0 || collapsedSections[id]) {
      return undefined
    }

    return visibleSectionIds
      .slice(startIndex + 1)
      .find(sectionId => !collapsedSections[sectionId])
  }

  return (
    <aside className={styles.sourceDebugPanel} aria-label="Source trace state">
      {step.exception && (
        <SourceDebugResizableSection
          id="exception"
          title="Exception"
          icon={<Bug size={14} aria-hidden="true" />}
          collapsed={collapsedSections.exception}
          height={sectionHeights.exception}
          grow={true}
          resizePartnerId={resizePartnerId("exception")}
          canResize={resizePartnerId("exception") !== undefined}
          onToggle={toggleSection}
          onResizeStart={startSectionResize}
          setSectionElement={setSectionElement}
        >
          <div className={styles.sourceDebugException}>
            <div className={styles.sourceDebugExceptionText}>
              {step.exception.symbolic_name ?? `Exit code ${step.exception.errno}`}
            </div>
          </div>
        </SourceDebugResizableSection>
      )}

      <SourceDebugResizableSection
        id="locals"
        title="Locals"
        icon={<Bug size={14} aria-hidden="true" />}
        collapsed={collapsedSections.locals}
        height={sectionHeights.locals}
        grow={true}
        resizePartnerId={resizePartnerId("locals")}
        canResize={resizePartnerId("locals") !== undefined}
        onToggle={toggleSection}
        onResizeStart={startSectionResize}
        setSectionElement={setSectionElement}
      >
        <SourceVariableList variables={step.locals} />
      </SourceDebugResizableSection>

      <SourceDebugResizableSection
        id="callStack"
        title="Call Stack"
        icon={<ListTree size={14} aria-hidden="true" />}
        collapsed={collapsedSections.callStack}
        height={sectionHeights.callStack}
        grow={true}
        resizePartnerId={resizePartnerId("callStack")}
        canResize={resizePartnerId("callStack") !== undefined}
        onToggle={toggleSection}
        onResizeStart={startSectionResize}
        setSectionElement={setSectionElement}
      >
        {step.call_stack.length > 0 ? (
          <div className={styles.sourceCallStack}>
            {step.call_stack.map((frame, index) => (
              <button
                key={`${frame.function_name}-${index}`}
                type="button"
                className={`${styles.sourceCallFrame} ${
                  index === activeCallFrameIndex ? styles.sourceCallFrameActive : ""
                }`}
                onClick={() => onCallFrameSelect(index)}
              >
                <SourceCallFrameIcon />
                <span className={styles.sourceCallFrameName}>{callFrameLabel(frame)}</span>
                {frame.location && (
                  <span className={styles.sourceCallFrameLocation}>
                    {sourceLocationWithColumnLabel(frame.location)}
                  </span>
                )}
              </button>
            ))}
          </div>
        ) : (
          <div className={styles.sourceDebugEmpty}>No frames</div>
        )}
      </SourceDebugResizableSection>

    </aside>
  )
}

function SourceFilesEditor({
  bundles,
  traceId,
  sourceTrace,
}: {
  readonly bundles: readonly SourceBundle[]
  readonly traceId: string
  readonly sourceTrace?: SourceTraceResponse
}) {
  const [activeBundleHash, setActiveBundleHash] = useState(bundles[0]?.source_bundle_hash ?? "")
  const activeBundle =
    bundles.find(bundle => bundle.source_bundle_hash === activeBundleHash) ?? bundles[0]
  const [activePath, setActivePath] = useState(activeBundle ? defaultSourcePath(activeBundle) : "")
  const activeFile = activeBundle
    ? findSourceFile(activeBundle.files, activePath) ?? activeBundle.files[0]
    : undefined
  const code = activeFile ? decodeSourceFile(activeFile) : ""
  const activeSourceTrace =
    activeBundle && sourceTrace?.source_bundle_hash === activeBundle.source_bundle_hash
      ? sourceTrace
      : undefined
  const sourceSteps = activeSourceTrace?.steps ?? []
  const [activeSourceStepIndex, setActiveSourceStepIndex] = useState(0)
  const [selectedCallFrameIndex, setSelectedCallFrameIndex] = useState<number | null>(null)
  const [shouldCenterSourceStep, setShouldCenterSourceStep] = useState(false)
  const [sourceDebugPanelWidth, setSourceDebugPanelWidth] = useState(() =>
    Math.min(
      SOURCE_DEBUG_PANEL_MAX_WIDTH,
      Math.max(
        SOURCE_DEBUG_PANEL_MIN_WIDTH,
        getStoredSourceDebugPanelWidth(SOURCE_DEBUG_PANEL_DEFAULT_WIDTH),
      ),
    ),
  )
  const sourceDebugLayoutRef = useRef<HTMLDivElement | null>(null)
  const currentSourceStep = sourceSteps[Math.min(activeSourceStepIndex, sourceSteps.length - 1)]
  const currentSourceStepIndex = currentSourceStep
    ? Math.min(activeSourceStepIndex, sourceSteps.length - 1)
    : 0
  const selectedCallFrameLocation =
    selectedCallFrameIndex !== null
      ? (currentSourceStep?.call_stack[selectedCallFrameIndex]?.location ?? null)
      : null
  const highlightedSourceLine =
    currentSourceStep &&
    activeFile &&
    activeBundle &&
    normalizeSourcePath(currentSourceStep.location.file) === normalizeSourcePath(activeFile.path)
      ? currentSourceStep.location.line
      : undefined
  const selectedCallFrameLine =
    selectedCallFrameLocation &&
    activeFile &&
    activeBundle &&
    normalizeSourcePath(selectedCallFrameLocation.file) === normalizeSourcePath(activeFile.path)
      ? selectedCallFrameLocation.line
      : undefined
  const shouldHighlightSelectedCallFrame =
    selectedCallFrameLine !== undefined && selectedCallFrameLine !== highlightedSourceLine
  const frameHighlightGroups =
    shouldHighlightSelectedCallFrame
      ? [
          {
            lines: [selectedCallFrameLine],
            color: "#5b7ebe",
            className: "source-debug-frame-line",
          },
        ]
      : []
  const centerSourceLine = selectedCallFrameLine ?? highlightedSourceLine

  useEffect(() => {
    const nextBundle = bundles[0]
    setActiveBundleHash(nextBundle?.source_bundle_hash ?? "")
    setActivePath(nextBundle ? defaultSourcePath(nextBundle) : "")
  }, [bundles])

  useEffect(() => {
    setActiveSourceStepIndex(0)
  }, [activeSourceTrace?.source_bundle_hash])

  useEffect(() => {
    setSelectedCallFrameIndex(null)
  }, [activeSourceTrace?.source_bundle_hash, currentSourceStepIndex])

  useEffect(() => {
    if (sourceSteps.length > 0 && activeSourceStepIndex >= sourceSteps.length) {
      setActiveSourceStepIndex(sourceSteps.length - 1)
    }
  }, [activeSourceStepIndex, sourceSteps.length])

  useEffect(() => {
    const locationToShow = selectedCallFrameLocation ?? currentSourceStep?.location
    if (!activeBundle || !locationToShow) {
      return
    }
    const sourceFile = findSourceFile(activeBundle.files, locationToShow.file)
    if (sourceFile) {
      setActivePath(sourceFile.path)
    }
  }, [activeBundle, currentSourceStep, selectedCallFrameLocation])

  const selectBundle = (bundle: SourceBundle) => {
    setActiveBundleHash(bundle.source_bundle_hash)
    setActivePath(defaultSourcePath(bundle))
  }

  const goToSourceStep = useCallback((step: number) => {
    const lastStep = Math.max(0, sourceSteps.length - 1)
    setShouldCenterSourceStep(true)
    setActiveSourceStepIndex(Math.max(0, Math.min(step, lastStep)))
  }, [sourceSteps.length])

  const handleSourceFileSelect = (path: string) => {
    setShouldCenterSourceStep(false)
    setSelectedCallFrameIndex(null)
    setActivePath(path)
  }

  const handleCallFrameSelect = useCallback(
    (index: number) => {
      const frameLocation = currentSourceStep?.call_stack[index]?.location
      if (!activeBundle || !frameLocation) {
        return
      }

      const sourceFile = findSourceFile(activeBundle.files, frameLocation.file)
      if (sourceFile) {
        setActivePath(sourceFile.path)
      }
      setSelectedCallFrameIndex(index)
      setShouldCenterSourceStep(true)
    },
    [activeBundle, currentSourceStep],
  )

  const startSourceDebugPanelResize = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      const layoutElement = sourceDebugLayoutRef.current
      if (!layoutElement) {
        return
      }

      event.preventDefault()

      const startX = event.clientX
      const startWidth = sourceDebugPanelWidth
      const layoutWidth = layoutElement.getBoundingClientRect().width
      const previousCursor = document.body.style.cursor
      const previousUserSelect = document.body.style.userSelect
      let latestWidth = startWidth
      document.body.style.cursor = "col-resize"
      document.body.style.userSelect = "none"

      const handlePointerMove = (moveEvent: globalThis.PointerEvent) => {
        const nextWidth = clampSourceDebugPanelWidth(
          Math.round(startWidth - (moveEvent.clientX - startX)),
          layoutWidth,
        )
        latestWidth = nextWidth
        setSourceDebugPanelWidth(current => (current === nextWidth ? current : nextWidth))
      }

      const finishResize = () => {
        document.body.style.cursor = previousCursor
        document.body.style.userSelect = previousUserSelect
        setStoredSourceDebugPanelWidth(latestWidth)
        window.removeEventListener("pointermove", handlePointerMove)
        window.removeEventListener("pointerup", finishResize)
        window.removeEventListener("pointercancel", finishResize)
      }

      window.addEventListener("pointermove", handlePointerMove)
      window.addEventListener("pointerup", finishResize)
      window.addEventListener("pointercancel", finishResize)
    },
    [sourceDebugPanelWidth],
  )

  useEffect(() => {
    if (!activeSourceTrace || !sourceDebugLayoutRef.current) {
      return
    }

    const layoutElement = sourceDebugLayoutRef.current
    const clampToLayout = () => {
      const layoutWidth = layoutElement.getBoundingClientRect().width
      setSourceDebugPanelWidth(current => clampSourceDebugPanelWidth(current, layoutWidth))
    }
    const observer = new ResizeObserver(clampToLayout)

    clampToLayout()
    observer.observe(layoutElement)

    return () => observer.disconnect()
  }, [activeSourceTrace])

  useEffect(() => {
    if (!activeSourceTrace || sourceSteps.length === 0) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey ||
        shouldIgnoreSourceDebugKey(event.target)
      ) {
        return
      }

      if (event.key === "ArrowLeft") {
        event.preventDefault()
        goToSourceStep(currentSourceStepIndex - 1)
      } else if (event.key === "ArrowRight") {
        event.preventDefault()
        goToSourceStep(currentSourceStepIndex + 1)
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [activeSourceTrace, currentSourceStepIndex, goToSourceStep, sourceSteps.length])

  if (!activeBundle || !activeFile) {
    return <div className={styles.emptySourceState}>No verified source files</div>
  }

  const editorModelPath = `retrace-source-${traceId}/${activeBundle.source_bundle_hash}/${normalizeSourcePath(activeFile.path)}`
  const sourceDebugLayoutStyle = activeSourceTrace
    ? ({
        "--source-debug-panel-width": `${sourceDebugPanelWidth}px`,
      } as CSSProperties)
    : undefined

  return (
    <section className={styles.sourceShell} aria-label="Verified source files">
      {bundles.length > 1 && (
        <div className={styles.sourceBundleTabs} role="tablist" aria-label="Source bundles">
          {bundles.map(bundle => (
            <button
              key={bundle.source_bundle_hash}
              type="button"
              className={`${styles.sourceBundleTab} ${
                bundle.source_bundle_hash === activeBundle.source_bundle_hash
                  ? styles.sourceBundleTabActive
                  : ""
              }`}
              onClick={() => selectBundle(bundle)}
              title={bundle.source_bundle_hash}
              role="tab"
              aria-selected={bundle.source_bundle_hash === activeBundle.source_bundle_hash}
            >
              {bundle.source_bundle_hash.slice(0, 10)}
            </button>
          ))}
        </div>
      )}

      {activeSourceTrace && (
        <SourceDebugToolbar
          selectedStep={currentSourceStepIndex}
          totalSteps={sourceSteps.length}
          currentStep={currentSourceStep}
          truncated={activeSourceTrace.truncated}
          onFirst={() => goToSourceStep(0)}
          onPrev={() => goToSourceStep(currentSourceStepIndex - 1)}
          onNext={() => goToSourceStep(currentSourceStepIndex + 1)}
          onLast={() => goToSourceStep(sourceSteps.length - 1)}
        />
      )}

      <div className={styles.sourceFileTabs} role="tablist" aria-label="Source files">
        {activeBundle.files.map(file => (
          <button
            key={file.path}
            type="button"
            className={`${styles.sourceFileTab} ${
              file.path === activeFile.path ? styles.sourceFileTabActive : ""
            }`}
            onClick={() => handleSourceFileSelect(file.path)}
            title={file.path}
            role="tab"
            aria-selected={file.path === activeFile.path}
          >
            <span className={styles.sourceFileTabName}>{sourceFileLabel(file.path)}</span>
          </button>
        ))}
      </div>

      <div
        ref={sourceDebugLayoutRef}
        className={`${styles.sourceDebugLayout} ${
          activeSourceTrace ? "" : styles.sourceDebugLayoutSingle
        }`}
        style={sourceDebugLayoutStyle}
      >
        <div className={styles.sourceEditorPane}>
          <Suspense
            fallback={
              <div className={styles.editorLoader}>
                <InlineLoader message="Loading editor" loading={true} />
              </div>
            }
          >
            <CodeEditor
              code={code}
              language={sourceLanguage(activeFile.path)}
              modelPath={editorModelPath}
              highlightLine={highlightedSourceLine}
              highlightGroups={frameHighlightGroups}
              shouldCenter={shouldCenterSourceStep && centerSourceLine !== undefined}
              centerLine={centerSourceLine}
              needBorderRadius={false}
              needFloatingTip={false}
              showInstructionDocs={false}
              compactGutter
              sourceDebugVariables={currentSourceStep?.locals ?? []}
            />
          </Suspense>
        </div>
        {activeSourceTrace && (
          <div
            className={styles.sourceDebugPanelResizeHandle}
            role="separator"
            aria-label="Resize debug panel"
            aria-orientation="vertical"
            aria-valuemin={SOURCE_DEBUG_PANEL_MIN_WIDTH}
            aria-valuemax={SOURCE_DEBUG_PANEL_MAX_WIDTH}
            aria-valuenow={sourceDebugPanelWidth}
            onPointerDown={startSourceDebugPanelResize}
          />
        )}
        {activeSourceTrace && (
          <SourceDebugPanel
            step={currentSourceStep}
            selectedCallFrameIndex={selectedCallFrameIndex}
            onCallFrameSelect={handleCallFrameSelect}
          />
        )}
      </div>
    </section>
  )
}

function RetraceWorkspaceFc({result, className}: RetraceWorkspaceProps) {
  const [selectedStackItem, setSelectedStackItem] = useState<{
    element: StackElement
    title: string
  } | null>(null)
  const [traceViewMode, setTraceViewMode] = useState<TraceViewMode>(() => getStoredTraceViewMode())
  const [workspaceTab, setWorkspaceTab] = useState<WorkspaceTab>("trace")

  const lineExecutionData = useLineExecutionData(result.trace)
  const {
    selectedStep,
    highlightLine,
    currentStep,
    currentStack,
    goToStep,
    handlePrev,
    handleNext,
    goToFirstStep,
    goToLastStep,
    canGoPrev,
    canGoNext,
    findStepByLine,
    transitionType,
    totalSteps,
  } = useTraceStepper(result.trace)

  const instructionDetails = useMemo(() => buildInstructionDetails(result), [result])
  const cumulativeGasSinceBegin = useMemo(
    () => calculateCumulativeGasSinceBegin(result.trace, selectedStep),
    [result.trace, selectedStep],
  )
  const implicitRet = useMemo(
    () => getImplicitRet(result.trace, selectedStep),
    [result.trace, selectedStep],
  )
  const stateUpdateHashOk = result.result.stateUpdateHashOk
  const sourceBundles = useMemo(
    () =>
      result.verifiedSource?.bundles
        .map(visibleSourceBundle)
        .filter(bundle => bundle.files.length > 0) ?? [],
    [result.verifiedSource],
  )
  const hasSourceBundles = sourceBundles.length > 0

  const handleTraceViewModeChange = useCallback((mode: TraceViewMode) => {
    setTraceViewMode(mode)
    setStoredTraceViewMode(mode)
  }, [])

  const handleWorkspaceTabChange = useCallback((tab: WorkspaceTab) => {
    setWorkspaceTab(tab)
    if (tab === "sources") {
      setSelectedStackItem(null)
    }
  }, [])

  const handleStackItemClick = useCallback(
    (element: StackElement, title: string) => {
      if (
        selectedStackItem &&
        selectedStackItem.element === element &&
        selectedStackItem.title === title
      ) {
        setSelectedStackItem(null)
        return
      }

      setSelectedStackItem({element, title})
    },
    [selectedStackItem],
  )

  useEffect(() => {
    if (!hasSourceBundles && workspaceTab === "sources") {
      setWorkspaceTab("trace")
    }
  }, [hasSourceBundles, workspaceTab])

  return (
    <section className={`${styles.root} ${className ?? ""}`} aria-label="Transaction retrace">
      <div className={styles.toolbar}>
        <div className={styles.toolbarLeft}>
          <div className={styles.primaryTabs} role="tablist" aria-label="Retrace views">
            <button
              type="button"
              className={`${styles.primaryTab} ${
                workspaceTab === "trace" ? styles.primaryTabActive : ""
              }`}
              onClick={() => handleWorkspaceTabChange("trace")}
              role="tab"
              aria-selected={workspaceTab === "trace"}
            >
              <Braces size={16} aria-hidden="true" />
              Trace
            </button>
            {hasSourceBundles && (
              <button
                type="button"
                className={`${styles.primaryTab} ${
                  workspaceTab === "sources" ? styles.primaryTabActive : ""
                }`}
                onClick={() => handleWorkspaceTabChange("sources")}
                role="tab"
                aria-selected={workspaceTab === "sources"}
              >
                <FileCode2 size={16} aria-hidden="true" />
                Sources
              </button>
            )}
          </div>

          {workspaceTab === "trace" && (
            <TraceViewModeToggle value={traceViewMode} onChange={handleTraceViewModeChange} />
          )}

          {stateUpdateHashOk === false && (
            <div className={styles.statusContainer} role="status" aria-live="polite">
              <Tooltip
                content={
                  "Because the transaction runs in a local sandbox, we can't always reproduce it exactly. Sandbox replay was incomplete, and some values may differ from those on the real blockchain."
                }
                placement="bottom"
              >
                <StatusBadge type="warning" text="Trace Incomplete" />
              </Tooltip>
            </div>
          )}
        </div>
      </div>

      <div
        className={`${styles.workspace} ${workspaceTab === "sources" ? styles.sourcesWorkspace : ""}`}
      >
        <section className={styles.codeArea} aria-label="Trace code">
          <div
            className={`${styles.codeEditorWrapper} ${
              workspaceTab === "trace" && selectedStackItem ? styles.codeEditorHidden : ""
            }`}
          >
            {workspaceTab === "sources" ? (
              <SourceFilesEditor
                bundles={sourceBundles}
                traceId={result.result.emulatedTx.lt.toString()}
                sourceTrace={result.sourceTrace}
              />
            ) : traceViewMode === "assembler" ? (
              <Suspense
                fallback={
                  <div className={styles.editorLoader}>
                    <InlineLoader message="Loading editor" loading={true} />
                  </div>
                }
              >
                <CodeEditor
                  code={result.code}
                  modelPath={`retrace-${result.result.emulatedTx.lt.toString()}.tasm`}
                  highlightLine={highlightLine}
                  implicitRetLine={implicitRet.line}
                  implicitRetLabel={
                    implicitRet.approx ? "implicit RET (approximate position)" : undefined
                  }
                  lineExecutionData={lineExecutionData}
                  onLineClick={findStepByLine}
                  shouldCenter={transitionType === "button"}
                  exitCode={result.exitCode}
                  needBorderRadius={false}
                  compactGutter
                />
              </Suspense>
            ) : (
              <TraceStepsChainView
                steps={instructionDetails}
                selectedStep={selectedStep}
                onStepClick={goToStep}
              />
            )}
          </div>

          {workspaceTab === "trace" && selectedStackItem && (
            <div className={styles.stackItemOverlay}>
              <StackItemDetails
                itemData={selectedStackItem.element}
                title={selectedStackItem.title}
                onClose={() => setSelectedStackItem(null)}
              />
            </div>
          )}
        </section>

        {workspaceTab === "trace" && (
          <TraceSidePanel
            selectedStep={selectedStep}
            totalSteps={totalSteps}
            currentStep={currentStep}
            currentStack={currentStack}
            canGoPrev={canGoPrev}
            canGoNext={canGoNext}
            onPrev={handlePrev}
            onNext={handleNext}
            onFirst={goToFirstStep}
            onLast={goToLastStep}
            showGas={true}
            placeholderMessage="No trace steps available."
            instructionDetails={instructionDetails}
            cumulativeGas={cumulativeGasSinceBegin}
            onStackItemClick={handleStackItemClick}
            className={styles.sidePanel}
          />
        )}
      </div>
    </section>
  )
}

const RetraceWorkspace = memo(RetraceWorkspaceFc)
RetraceWorkspace.displayName = "RetraceWorkspace"

export default RetraceWorkspace
