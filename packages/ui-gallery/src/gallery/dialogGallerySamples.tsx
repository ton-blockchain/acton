import {Button, Dialog, HighlightedCode, RawDataBlock} from "@acton/ui"
import {useState} from "react"

import styles from "./dialogGallery.module.css"

const metadataJson = JSON.stringify(
  {
    address: "0:b113a994b5024a16719f69139328eb759596c38a25f59028b146fecdc3621dfe",
    decimals: "6",
    name: "Tether USD",
    symbol: "USD₮",
  },
  undefined,
  2,
)
const diagnosticEntries = Array.from({length: 18}, (_, index) => `Diagnostic entry ${index + 1}`)

export function DialogGallerySamples() {
  const [standardOpen, setStandardOpen] = useState(false)
  const [scrollingOpen, setScrollingOpen] = useState(false)

  return (
    <div className={styles.sampleGrid}>
      <article className={styles.sample}>
        <div className={styles.sampleText}>
          <h4>Inspection dialog</h4>
          <p>
            A compact modal combines structured details with existing technical-data components.
          </p>
        </div>
        <Button variant="secondary" onClick={() => setStandardOpen(true)}>
          Open dialog
        </Button>
        <Dialog
          open={standardOpen}
          onOpenChange={setStandardOpen}
          title="Metadata"
          maxWidth="38rem"
        >
          <div className={styles.dialogContent}>
            <div className={styles.identity}>
              <span className={styles.avatar} aria-hidden="true">
                T
              </span>
              <div>
                <h3>Tether USD</h3>
                <p>Tether Token for Tether USD</p>
              </div>
            </div>
            <dl className={styles.details}>
              <div>
                <dt>Symbol</dt>
                <dd>USD₮</dd>
              </div>
              <div>
                <dt>Mintable</dt>
                <dd>true</dd>
              </div>
            </dl>
            <RawDataBlock
              title="Raw metadata"
              value={metadataJson}
              copyLabel="metadata JSON"
              customContent={<HighlightedCode value={metadataJson} language="json" />}
            />
          </div>
        </Dialog>
      </article>

      <article className={styles.sample}>
        <div className={styles.sampleText}>
          <h4>Long content</h4>
          <p>The shared frame stays inside the viewport while only the dialog content scrolls.</p>
        </div>
        <Button variant="outline" onClick={() => setScrollingOpen(true)}>
          Open long dialog
        </Button>
        <Dialog
          open={scrollingOpen}
          onOpenChange={setScrollingOpen}
          title="Trace diagnostics"
          description="A deliberately long example for viewport and focus checks."
          maxWidth="34rem"
        >
          <ol className={styles.longList}>
            {diagnosticEntries.map(entry => (
              <li key={entry}>{entry}</li>
            ))}
          </ol>
        </Dialog>
      </article>
    </div>
  )
}
