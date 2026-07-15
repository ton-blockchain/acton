import {MarkdownText, ThemeSwitch, ToastProvider, cx} from "@acton/ui"
import {PanelLeftOpen, X} from "lucide-react"
import {useEffect, useState} from "react"

import styles from "./App.module.css"
import {galleries} from "./gallery/registry"
import type {ComponentGallery} from "./gallery/types"

type Theme = "light" | "dark"

const initialGallery = galleries.find(gallery => gallery.id === "button") ?? galleries[0]
const galleryParamName = "component"
const themeStorageKey = "acton-ui-gallery-theme"

export function App() {
  const [theme, setTheme] = useState<Theme>(getInitialTheme)
  const [activeGalleryId, setActiveGalleryId] = useState(getInitialGalleryId)
  const [isNavigationOpen, setIsNavigationOpen] = useState(false)

  const activeGallery: ComponentGallery =
    galleries.find(gallery => gallery.id === activeGalleryId) ?? initialGallery

  useEffect(() => {
    if (!isNavigationOpen) return

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setIsNavigationOpen(false)
    }
    const previousOverflow = document.body.style.overflow

    document.body.style.overflow = "hidden"
    document.addEventListener("keydown", handleKeyDown)

    return () => {
      document.body.style.overflow = previousOverflow
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [isNavigationOpen])

  const toggleTheme = () => {
    setTheme(currentTheme => {
      const nextTheme = currentTheme === "light" ? "dark" : "light"
      globalThis.localStorage?.setItem(themeStorageKey, nextTheme)
      return nextTheme
    })
  }

  const selectGallery = (galleryId: string) => {
    setActiveGalleryId(galleryId)
    setIsNavigationOpen(false)
    updateGalleryUrl(galleryId)
  }

  return (
    <ToastProvider theme={theme}>
      <div className={styles.shell} data-theme={theme}>
        <header className={styles.mobileToolbar}>
          <button
            type="button"
            className={styles.menuButton}
            aria-label="Open gallery navigation"
            aria-controls="gallery-navigation-panel"
            aria-expanded={isNavigationOpen}
            onClick={() => setIsNavigationOpen(true)}
          >
            <PanelLeftOpen size={18} aria-hidden="true" />
          </button>
          <div className={styles.mobileTitle}>
            <span>Acton UI</span>
            <strong>{activeGallery.title}</strong>
          </div>
          <ThemeSwitch
            theme={theme}
            onToggleTheme={toggleTheme}
            aria-label={theme === "dark" ? "Use light theme" : "Use dark theme"}
          />
        </header>

        {isNavigationOpen ? (
          <button
            type="button"
            className={styles.backdrop}
            aria-label="Close gallery navigation"
            onClick={() => setIsNavigationOpen(false)}
          />
        ) : undefined}

        <aside
          id="gallery-navigation-panel"
          className={cx(styles.sidebar, isNavigationOpen && styles.sidebarOpen)}
        >
          <div className={styles.sidebarHeader}>
            <div className={styles.sidebarHeaderText}>
              <p className={styles.eyebrow}>Acton UI</p>
              <h1 className={styles.title}>UI gallery</h1>
              <p className={styles.sidebarText}>
                Visual inventory for Acton foundations, reusable primitives, variants, and states.
              </p>
            </div>
            <button
              type="button"
              className={styles.drawerClose}
              aria-label="Close gallery navigation"
              onClick={() => setIsNavigationOpen(false)}
            >
              <X size={17} aria-hidden="true" />
            </button>
          </div>

          <nav className={styles.navigation} aria-label="Gallery pages">
            {galleries.map(gallery => (
              <button
                key={gallery.id}
                type="button"
                className={cx(
                  styles.navigationItem,
                  gallery.id === activeGallery.id && styles.navigationItemActive,
                )}
                onClick={() => selectGallery(gallery.id)}
              >
                <span>{gallery.title}</span>
                <span className={styles.navigationStatus}>{gallery.status}</span>
              </button>
            ))}
          </nav>

          <div className={styles.sidebarFooter}>
            <ThemeSwitch
              theme={theme}
              onToggleTheme={toggleTheme}
              aria-label={theme === "dark" ? "Use light theme" : "Use dark theme"}
            />
          </div>
        </aside>

        <main className={styles.main}>
          <header className={styles.componentHeader}>
            <div className={styles.componentHeaderText}>
              <p className={styles.eyebrow}>
                {activeGallery.kind === "foundation" ? "Foundation" : "Component"}
              </p>
              <h2 className={styles.componentTitle}>{activeGallery.title}</h2>
              <p className={styles.componentSummary}>{activeGallery.summary}</p>
            </div>
          </header>

          {activeGallery.kind === "foundation" ? undefined : (
            <section className={styles.notesGrid} aria-label="Usage guidance">
              <article className={styles.noteBlock}>
                <h3>Use When</h3>
                <ul>
                  {activeGallery.usage.map(item => (
                    <li key={item}>
                      <MarkdownText className={styles.noteText} tone="muted">
                        {item}
                      </MarkdownText>
                    </li>
                  ))}
                </ul>
              </article>
              <article className={styles.noteBlock}>
                <h3>Avoid When</h3>
                <ul>
                  {activeGallery.avoid.map(item => (
                    <li key={item}>
                      <MarkdownText className={styles.noteText} tone="muted">
                        {item}
                      </MarkdownText>
                    </li>
                  ))}
                </ul>
              </article>
              <article className={styles.agentBlock}>
                <h3>Agent Note</h3>
                <MarkdownText className={styles.agentText} tone="muted">
                  {activeGallery.agentSummary}
                </MarkdownText>
                <code>{activeGallery.importStatement}</code>
              </article>
            </section>
          )}

          <div className={styles.sections}>
            {activeGallery.sections.map(section => (
              <section
                key={section.id}
                className={styles.gallerySection}
                aria-labelledby={section.id}
              >
                <div className={styles.sectionHeader}>
                  <h3 id={section.id}>{section.title}</h3>
                  {section.description ? <p>{section.description}</p> : undefined}
                </div>
                {section.content}
              </section>
            ))}
          </div>
        </main>
      </div>
    </ToastProvider>
  )
}

function getInitialTheme(): Theme {
  const storedTheme = globalThis.localStorage?.getItem(themeStorageKey)
  return storedTheme === "dark" ? "dark" : "light"
}

function getInitialGalleryId() {
  const params = new URLSearchParams(globalThis.location?.search)
  const galleryId = params.get(galleryParamName)
  return galleries.some(gallery => gallery.id === galleryId) ? galleryId : initialGallery.id
}

function updateGalleryUrl(galleryId: string) {
  const url = new URL(globalThis.location.href)

  if (galleryId === initialGallery.id) {
    url.searchParams.delete(galleryParamName)
  } else {
    url.searchParams.set(galleryParamName, galleryId)
  }

  globalThis.history.replaceState(null, "", url)
}
