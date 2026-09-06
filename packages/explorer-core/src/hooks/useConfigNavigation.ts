import {useEffect, useRef, useState} from "react"

import type {NetworkConfigParameter} from "../api/config"

/**
 * Tracks the current parameter while scrolling the page or an embedded container.
 *
 * Scrolling replaces the URL fragment without adding browser history entries.
 * Explicit anchor clicks add history entries so Back returns to the previous parameter.
 */
export function useConfigNavigation(parameters: readonly NetworkConfigParameter[]) {
  const contentRef = useRef<HTMLDivElement>(null)
  const indexRef = useRef<HTMLElement>(null)
  const restoredAnchor = useRef(false)
  const [activeId, setActiveId] = useState<number>()

  useEffect(() => {
    const content = contentRef.current
    if (!content) return

    let scrollRoot = content.parentElement
    while (scrollRoot && !/(auto|scroll)/.test(getComputedStyle(scrollRoot).overflowY)) {
      scrollRoot = scrollRoot.parentElement
    }
    const scrollTarget = scrollRoot ?? globalThis
    const scrollArea = scrollRoot ?? document.documentElement
    const cards = parameters.flatMap(parameter => {
      const element = document.getElementById(`config-parameter-${parameter.id}`)
      return element && content.contains(element) ? [{id: parameter.id, element}] : []
    })
    let frame = 0

    const update = () => {
      frame = 0
      content.style.setProperty("--config-scroll-height", `${scrollArea.clientHeight}px`)
      const readingEdge = (scrollRoot?.getBoundingClientRect().top ?? 0) + 16
      let current: (typeof cards)[number] | undefined = cards[0]
      for (const card of cards) {
        if (card.element.getBoundingClientRect().top > readingEdge + 1) break
        current = card
      }
      // Short final cards cannot reach the reading edge when scrolling stops.
      if (
        scrollArea.scrollHeight > scrollArea.clientHeight + 1 &&
        scrollArea.scrollTop + scrollArea.clientHeight >= scrollArea.scrollHeight - 1
      ) {
        current = cards.at(-1)
      }
      setActiveId(current?.id)

      const hash = current ? `#config-parameter-${current.id}` : ""
      if (location.hash !== hash) {
        history.replaceState(history.state, "", `${location.pathname}${location.search}${hash}`)
      }
    }

    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update)
    }
    const restore = () => {
      try {
        const anchor = decodeURIComponent(location.hash.slice(1))
        const card = cards.find(candidate => candidate.element.id === anchor)
        card?.element.scrollIntoView({behavior: "instant", block: "start"})
      } catch {
        // An invalid fragment must not prevent normal scrolling or filtering.
      }
      schedule()
    }

    if (!restoredAnchor.current) {
      restoredAnchor.current = true
      restore()
    }
    schedule()
    scrollTarget.addEventListener("scroll", schedule, {passive: true})
    globalThis.addEventListener("resize", schedule)
    globalThis.addEventListener("popstate", restore)
    globalThis.addEventListener("hashchange", restore)
    const observer = new ResizeObserver(schedule)
    observer.observe(content)
    if (scrollRoot) observer.observe(scrollRoot)

    return () => {
      cancelAnimationFrame(frame)
      observer.disconnect()
      scrollTarget.removeEventListener("scroll", schedule)
      globalThis.removeEventListener("resize", schedule)
      globalThis.removeEventListener("popstate", restore)
      globalThis.removeEventListener("hashchange", restore)
    }
  }, [parameters])

  useEffect(() => {
    const panel = indexRef.current
    const link = panel?.querySelector<HTMLElement>('[aria-current="location"]')
    if (!panel || !link) return

    const panelRect = panel.getBoundingClientRect()
    const linkRect = link.getBoundingClientRect()
    const delta =
      linkRect.top < panelRect.top
        ? linkRect.top - panelRect.top - 8
        : linkRect.bottom > panelRect.bottom
          ? linkRect.bottom - panelRect.bottom + 8
          : 0
    if (delta) {
      panel.scrollBy({
        top: delta,
        behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth",
      })
    }
  }, [activeId])

  return {activeId, contentRef, indexRef}
}
