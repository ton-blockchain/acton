import {
  CHANGE_LIBRARY_MODE_CONSTANTS,
  ChangeLibraryModeViewer,
  ModeViewer,
  RESERVE_MODE_CONSTANTS,
  ReserveModeViewer,
  SEND_MODE_CONSTANTS,
  SendModeViewer,
  type ModeParser,
} from "@acton/ui"

import styles from "./modeViewerGallery.module.css"
import type {ComponentGallery} from "./types"

const customModeParser: ModeParser = mode => [
  {
    name: "CustomMode",
    value: mode,
    description: "A caller-provided mode parser can reuse the same presentation.",
  },
]

const samples = [
  {
    id: "reserve",
    title: "Reserve Mode",
    description: "Base reserve behavior plus optional balance and sign flags.",
    viewer: <ReserveModeViewer mode={12} />,
  },
  {
    id: "send",
    title: "Send Mode",
    description: "Independent send flags are combined into one inline value.",
    viewer: <SendModeViewer mode={3} />,
  },
  {
    id: "change-library",
    title: "Change Library Mode",
    description: "Library visibility and bounce behavior use their own parser.",
    viewer: <ChangeLibraryModeViewer mode={18} />,
  },
  {
    id: "unavailable",
    title: "Unavailable",
    description: "Every wrapper shares the same missing-value state.",
    viewer: <SendModeViewer mode={undefined} />,
  },
] as const

const flagGroups = [
  {
    id: "reserve-flags",
    title: "Reserve modes",
    description:
      "Base modes and optional flags used by reserveGramsOnBalance and reserveExtraCurrenciesOnBalance.",
    flags: Object.entries(RESERVE_MODE_CONSTANTS).map(([value, flag]) => ({
      name: flag.name,
      mode: Number(value),
      kind: Number(value) < 4 ? "base mode" : "base 0 + flag",
    })),
    render: (mode: number) => <ReserveModeViewer mode={mode} />,
  },
  {
    id: "send-flags",
    title: "Send modes",
    description: "Modes and flags accepted by OutMessage.send in the Tolk standard library.",
    flags: Object.entries(SEND_MODE_CONSTANTS).map(([value, flag]) => ({
      name: flag.name,
      mode: Number(value),
      kind: Number(value) === 0 ? "base mode" : "flag",
    })),
    render: (mode: number) => <SendModeViewer mode={mode} />,
  },
  {
    id: "change-library-flags",
    title: "Change library modes",
    description: "Base modes and the bounce-on-error flag for change-library actions.",
    flags: Object.values(CHANGE_LIBRARY_MODE_CONSTANTS).map(flag => ({
      name: flag.name,
      mode: flag.value,
      kind: flag.value < 3 ? "base mode" : "base 0 + flag",
    })),
    render: (mode: number) => <ChangeLibraryModeViewer mode={mode} />,
  },
] as const

function WrapperSamples() {
  return (
    <div className={styles.grid}>
      {samples.map(sample => (
        <article key={sample.id} className={styles.sample}>
          <div className={styles.sampleText}>
            <h4 className={styles.sampleTitle}>{sample.title}</h4>
            <p className={styles.sampleDescription}>{sample.description}</p>
          </div>
          {sample.viewer}
        </article>
      ))}
    </div>
  )
}

// react-doctor-disable-next-line react-doctor/no-multi-comp -- private examples belong to this gallery descriptor
function BaseComponentSample() {
  return (
    <article className={styles.sample}>
      <div className={styles.sampleText}>
        <h4 className={styles.sampleTitle}>Custom Parser</h4>
        <p className={styles.sampleDescription}>
          ModeViewer can render another mode family when it receives a compatible parser.
        </p>
      </div>
      <ModeViewer mode={7} parseMode={customModeParser} />
    </article>
  )
}

// react-doctor-disable-next-line react-doctor/no-multi-comp -- private examples belong to this gallery descriptor
function AllFlagsSamples() {
  return (
    <div className={styles.flagGroups}>
      {flagGroups.map(group => (
        <section key={group.id} className={styles.flagGroup}>
          <div className={styles.sampleText}>
            <h4 className={styles.sampleTitle}>{group.title}</h4>
            <p className={styles.sampleDescription}>{group.description}</p>
          </div>
          <div className={styles.flagGrid}>
            {group.flags.map(flag => (
              <article key={flag.name} className={styles.flagSample}>
                <span className={styles.flagMode}>
                  mode {flag.mode} · {flag.kind}
                </span>
                {group.render(flag.mode)}
              </article>
            ))}
          </div>
        </section>
      ))}
    </div>
  )
}

export const modeViewerGallery = {
  id: "mode-viewer",
  title: "ModeViewer",
  status: "ready",
  summary:
    "ModeViewer renders parsed bit flags consistently, while ReserveModeViewer, SendModeViewer, and ChangeLibraryModeViewer select domain-specific parsers.",
  importStatement:
    'import {ChangeLibraryModeViewer, ModeViewer, ReserveModeViewer, SendModeViewer} from "@acton/ui"',
  agentSummary:
    "Use the domain wrapper when rendering known TON action modes. Use ModeViewer directly only when introducing another parser with the same name, value, and description shape.",
  usage: [
    "Prefer ReserveModeViewer, SendModeViewer, or ChangeLibraryModeViewer for their matching TON action modes.",
    "Keep parsing rules in the domain parser file; ModeViewer owns only shared presentation.",
    "Pass undefined when a mode value is unavailable; every wrapper renders the same No mode state.",
  ],
  avoid: [
    "Do not duplicate flag separators, help popovers, or missing-value styling in callers.",
    "Do not put reserve, send, or library parsing branches inside the shared ModeViewer.",
    "Do not use the wrong domain wrapper merely because the numeric flags overlap.",
  ],
  sections: [
    {
      id: "mode-viewer-wrappers",
      title: "Domain Wrappers",
      description:
        "Each wrapper keeps its own parsing semantics and delegates the resulting flags to the same renderer.",
      content: <WrapperSamples />,
    },
    {
      id: "mode-viewer-all-flags",
      title: "All Modes and Flags",
      description:
        "Every supported value is rendered separately for checking its label, description, and documentation link.",
      content: <AllFlagsSamples />,
    },
    {
      id: "mode-viewer-base",
      title: "Base Component",
      description: "The shared renderer accepts any parser returning ModeInfo entries.",
      content: <BaseComponentSample />,
    },
  ],
} satisfies ComponentGallery
