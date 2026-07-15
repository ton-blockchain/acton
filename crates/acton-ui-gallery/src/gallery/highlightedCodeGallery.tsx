import {HighlightedCode} from "@acton/ui"

import styles from "./highlightedCodeGallery.module.css"
import type {ComponentGallery} from "./types"

const samples = [
  {
    id: "tolk",
    title: "Tolk",
    language: "tolk",
    value: `struct Transfer {
    queryId: uint64
    amount: coins
}

fun sendTransfer(message: Transfer): void {
    assert(message.amount > 0, 100)
}`,
  },
  {
    id: "func",
    title: "FunC",
    language: "func",
    value: `() recv_internal(int balance, int msg_value, cell in_msg_full, slice in_msg_body) {
    int op = in_msg_body~load_uint(32);
    throw_unless(100, op == 0x7362d09c);
}`,
  },
  {
    id: "tasm",
    title: "TVM assembly",
    language: "tasm",
    value: `SETCP 0
DICTPUSHCONST 19 [
  0 => {
    DUP
    SBITS
    LESSINT 32
  }
]`,
  },
  {
    id: "tlb",
    title: "TL-B",
    language: "tlb",
    value: `transfer#0f8a7ea5 query_id:uint64 amount:(VarUInteger 16)
destination:MsgAddress response_destination:MsgAddress
custom_payload:(Maybe ^Cell) = InternalMsgBody;`,
  },
  {
    id: "json",
    title: "JSON",
    language: "json",
    value: `{
  "name": "Acton",
  "verified": true,
  "exitCode": 0
}`,
  },
] as const

function LanguageSamples() {
  return (
    <div className={styles.grid}>
      {samples.map(sample => (
        <article key={sample.id} className={styles.sample}>
          <h4 className={styles.title}>{sample.title}</h4>
          <div className={styles.frame}>
            <HighlightedCode
              className={styles.code}
              value={sample.value}
              language={sample.language}
            />
          </div>
        </article>
      ))}
    </div>
  )
}

function BehaviorSamples() {
  const longLine =
    "fun buildMessage(destination: address, amount: coins, responseDestination: address, customPayload: cell?): cell"

  return (
    <div className={styles.behaviorGrid}>
      <article className={styles.sample}>
        <h4 className={styles.title}>Horizontal scroll</h4>
        <div className={styles.frame}>
          <HighlightedCode className={styles.code} value={longLine} language="tolk" wrap={false} />
        </div>
      </article>
      <article className={styles.sample}>
        <h4 className={styles.title}>Wrapped</h4>
        <div className={styles.frame}>
          <HighlightedCode className={styles.code} value={longLine} language="tolk" wrap />
        </div>
      </article>
      <article className={styles.sample}>
        <h4 className={styles.title}>Plain fallback</h4>
        <div className={styles.frame}>
          <HighlightedCode className={styles.code} value="B5EE9C7241010101002F00005A42" />
        </div>
      </article>
    </div>
  )
}

export const highlightedCodeGallery = {
  id: "highlighted-code",
  title: "HighlightedCode",
  status: "ready",
  summary:
    "HighlightedCode provides one read-only syntax-highlighting surface for Acton source code, schemas, assembly, and JSON.",
  importStatement: 'import {HighlightedCode} from "@acton/ui"',
  agentSummary:
    "Use HighlightedCode for read-only source code. Keep fetching, tabs, copy controls, line annotations, and editor behavior in surrounding domain components.",
  usage: [
    "Use for read-only Tolk, FunC, TVM assembly, TL-B, and JSON.",
    "Omit language for plain preformatted text that must share the same code geometry.",
    "Enable wrap for schemas and compact panels; keep it disabled for aligned assembly and logs.",
    "Compose it inside RawDataBlock when the code also needs a frame, title, collapse, or copy action.",
  ],
  avoid: [
    "Do not use as an editor; use the Monaco-based CodeEditor for editable or debug-aware source.",
    "Do not put file loading, ABI parsing, decompilation, or coverage logic inside this component.",
    "Do not create local Shiki instances or hand-color JSON and assembly tokens.",
  ],
  sections: [
    {
      id: "highlighted-code-languages",
      title: "Languages",
      description: "Every grammar owned by the shared highlighter.",
      content: <LanguageSamples />,
    },
    {
      id: "highlighted-code-behavior",
      title: "Wrapping And Plain Text",
      description: "The same geometry supports scrolling, wrapping, and unhighlighted fallback.",
      content: <BehaviorSamples />,
    },
  ],
} satisfies ComponentGallery
