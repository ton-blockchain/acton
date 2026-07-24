import {
  ArrowRight,
  Binary,
  Blocks,
  Braces,
  Bug,
  Code2,
  Database,
  FlaskConical,
  Github,
  History,
  Network,
  Search,
  Star,
  TerminalSquare,
  WalletCards,
} from "lucide-react"
import type {LucideIcon} from "lucide-react"
import type {FC} from "react"
import {Link} from "react-router-dom"

import styles from "./LandingPage.module.css"

interface LandingFeature {
  readonly eyebrow: string
  readonly title: string
  readonly highlight: string
  readonly description: string
  readonly detail: string
  readonly href: string
  readonly linkLabel: string
  readonly image: string
  readonly imageAlt: string
  readonly icon: LucideIcon
  readonly windowLabel: string
}

interface CompactFeature {
  readonly label: string
  readonly title: string
  readonly description: string
  readonly href: string
  readonly icon: LucideIcon
}

const LANDING_FEATURES: readonly LandingFeature[] = [
  {
    eyebrow: "Transaction debugging",
    title: "Find the phase that failed",
    highlight: "phase",
    description:
      "A red status is only the beginning. Follow the message route, value flow, state changes, and every transaction in the execution tree.",
    detail:
      "Exit codes, gas, bounced messages, and action phases stay together. Open Debug for a VM retrace, while state diffs are replayed and checked against the on-chain state update.",
    href: "/blocks",
    linkLabel: "Inspect live transactions",
    image: "/landing/failed-transaction-dark.png",
    imageAlt:
      "A failed transaction in actonscan with its message route, exit code, fees, and compute phase",
    icon: Bug,
    windowLabel: "Transaction execution",
  },
  {
    eyebrow: "Contract context",
    title: "Read ABI as developer docs",
    highlight: "ABI",
    description:
      "Browse get methods, message schemas, storage, declarations, and thrown errors as a readable contract interface instead of raw JSON.",
    detail:
      "Compiler metadata, source links, method IDs, and protocol documentation give every decoded value the context you need to use it correctly.",
    href: "/abi",
    linkLabel: "Browse the ABI catalog",
    image: "/landing/abi-detail-dark.png",
    imageAlt: "The STON.fi Router V2 ABI rendered as developer documentation in actonscan",
    icon: Braces,
    windowLabel: "Rendered contract ABI",
  },
  {
    eyebrow: "Cell-level inspection",
    title: "Go below the decoded value",
    highlight: "decoded",
    description:
      "Paste Base64, hex, a ton:// URL, or an explorer link. Inspect roots, bits, references, hashes, and the original BoC in one workspace.",
    detail:
      "Automatic ABI and TON-format detection gets you started quickly. Switch between parsed, raw-cell, and BoC views, use custom TL-B, or follow a code cell into source and disassembly.",
    href: "/cell",
    linkLabel: "Open Cell Inspector",
    image: "/landing/cell-inspector-dark.png",
    imageAlt:
      "Cell Inspector in actonscan showing a decoded BoC next to raw cell structure and hash metadata",
    icon: Binary,
    windowLabel: "Cell Inspector",
  },
  {
    eyebrow: "Message emulation",
    title: "Turn a trace into an experiment",
    highlight: "experiment",
    description:
      "Start with a real incoming message or build one from ABI. Change the BoC, value, timestamp, and network snapshot, then run it again.",
    detail:
      "Advanced options and state overrides make a failure reproducible without rebuilding the transaction by hand. The result comes back as the same readable execution tree.",
    href: "/emulate",
    linkLabel: "Open the message emulator",
    image: "/landing/emulator-dark.png",
    imageAlt:
      "The actonscan emulator builder showing decoded Jetton transfer fields, typed ABI values, and an editable destination",
    icon: TerminalSquare,
    windowLabel: "Message Emulator",
  },
]

const COMPACT_FEATURES: readonly CompactFeature[] = [
  {
    label: "VM replay",
    title: "Step through the failure",
    description:
      "Retrace TVM instructions, inspect stack state, and correlate execution with source metadata when it is available.",
    href: "/blocks",
    icon: Bug,
  },
  {
    label: "Accounts",
    title: "See a contract, not a balance",
    description:
      "Keep code, storage, get methods, token context, actions, and verified sources on one contract-first account page.",
    href: "/",
    icon: WalletCards,
  },
  {
    label: "Verification",
    title: "Connect code to chain",
    description:
      "Open verified contracts or register browser-local ABI, code-hash mappings, and source artifacts alongside compiler metadata.",
    href: "/verified",
    icon: Database,
  },
  {
    label: "Discovery",
    title: "Search developer primitives",
    description:
      "Jump to an address, .ton name, transaction hash, block, opcode, or a known ABI declaration.",
    href: "/",
    icon: Search,
  },
  {
    label: "Block tooling",
    title: "Navigate chain history",
    description:
      "Move by date, latest, previous, or next across masterchain and shard blocks, then inspect transactions or download raw BoC.",
    href: "/blocks",
    icon: History,
  },
  {
    label: "Your infrastructure",
    title: "Explore any TON network",
    description:
      "Switch between mainnet and testnet or attach v2/v3 endpoints for a compatible devnet, then share the network in a link.",
    href: "/",
    icon: Network,
  },
  {
    label: "Investigation",
    title: "Keep the cases that matter",
    description:
      "Favorite accounts and transactions, then return to balances, tokens, and traces without rebuilding your context.",
    href: "/favorites",
    icon: Star,
  },
  {
    label: "Testnet",
    title: "Fund the next experiment",
    description:
      "Request Testnet GRAM with the built-in browser PoW faucet and continue straight into deployment and debugging.",
    href: "/faucet",
    icon: FlaskConical,
  },
]

export const LandingPage: FC = () => (
  <div className={styles.page}>
    <title>actonscan · TON explorer for smart-contract developers</title>
    <meta
      name="description"
      content="Trace and replay TON transactions, decode contract ABI, inspect cells, emulate messages, and debug custom networks with actonscan."
    />

    <section className={styles.heroSection} aria-labelledby="actonscan-landing-title">
      <div className={styles.heroPanel}>
        <div className={styles.heroContent}>
          <div className={styles.heroCopy}>
            <span className={styles.productPill}>
              <Code2 size={15} aria-hidden="true" />
              Developer-first TON explorer
            </span>
            <h1 id="actonscan-landing-title" className={styles.heroTitle}>
              A TON explorer built for{" "}
              <span className={styles.accentText}>smart-contract developers</span>
            </h1>
            <p className={styles.heroDescription}>
              Actonscan turns transactions, messages, contract ABI, cells, and emulator output into
              one coherent debugging surface.
            </p>
            <p className={styles.heroStatement}>
              Follow execution. <span className={styles.accentText}>Not just balances.</span>
            </p>

            <div className={styles.heroActions}>
              <Link className={styles.primaryAction} to="/">
                Open actonscan
                <ArrowRight size={17} aria-hidden="true" />
              </Link>
              <Link className={styles.secondaryAction} to="/emulate">
                Start with the emulator
              </Link>
            </div>

            <dl className={styles.heroFacts}>
              <div>
                <dt>Networks</dt>
                <dd>Mainnet · Testnet · Devnet</dd>
              </div>
              <div>
                <dt>Built for</dt>
                <dd>Tracing · Retracing · Decoding · Emulation</dd>
              </div>
            </dl>
          </div>
        </div>

        <div className={styles.heroMedia} aria-hidden="true">
          <div className={styles.heroMediaFrame}>
            <img src="/landing/explorer-home-dark.png" alt="" />
          </div>
          <div className={styles.heroMediaFade} />
        </div>
      </div>
    </section>

    <section className={styles.featuresSection} aria-labelledby="landing-features-title">
      <div className={styles.sectionHeader}>
        <p className={styles.sectionEyebrow}>
          <span />
          Core workflows
          <span />
        </p>
        <h2 id="landing-features-title">
          The <span className={styles.accentText}>execution path</span>, made readable
        </h2>
        <p>Every layer you need to understand what a contract actually did.</p>
      </div>

      <div className={styles.featureList}>
        {LANDING_FEATURES.map((feature, index) => {
          const Icon = feature.icon
          const highlightedAt = feature.title.indexOf(feature.highlight)
          const reversed = index % 2 === 1

          return (
            <article
              className={`${styles.featureCard} ${reversed ? styles.featureCardReversed : ""}`}
              key={feature.title}
            >
              <div className={styles.featureCopy}>
                <div className={styles.featureCopyInner}>
                  <span className={styles.featureIcon}>
                    <Icon size={34} strokeWidth={1.7} aria-hidden="true" />
                  </span>
                  <p className={styles.featureEyebrow}>{feature.eyebrow}</p>
                  <h3>
                    {feature.title.slice(0, highlightedAt)}
                    <span className={styles.accentText}>{feature.highlight}</span>
                    {feature.title.slice(highlightedAt + feature.highlight.length)}
                  </h3>
                  <p>{feature.description}</p>
                  <p>{feature.detail}</p>
                  <Link className={styles.inlineAction} to={feature.href}>
                    {feature.linkLabel}
                    <ArrowRight size={16} aria-hidden="true" />
                  </Link>
                </div>
              </div>

              <div className={styles.featureMedia}>
                <div className={styles.windowBar}>
                  <span className={styles.windowDots} aria-hidden="true">
                    <i />
                    <i />
                    <i />
                  </span>
                  <span>{feature.windowLabel}</span>
                  <span className={styles.windowHost}>actonscan.com</span>
                </div>
                <img src={feature.image} alt={feature.imageAlt} />
              </div>
            </article>
          )
        })}
      </div>
    </section>

    <section className={styles.depthSection} aria-labelledby="landing-depth-title">
      <div className={styles.sectionHeader}>
        <p className={styles.sectionEyebrow}>
          <span />
          Full developer toolbox
          <span />
        </p>
        <h2 id="landing-depth-title">
          Stay in the <span className={styles.accentText}>debugging loop</span>
        </h2>
        <p>
          Trace, retrace, decode, emulate, inspect, and keep your investigation context in one
          explorer.
        </p>
      </div>

      <div className={styles.compactGrid}>
        {COMPACT_FEATURES.map(feature => {
          const Icon = feature.icon
          return (
            <Link className={styles.compactCard} to={feature.href} key={feature.title}>
              <div className={styles.compactCardTop}>
                <Icon size={24} strokeWidth={1.7} aria-hidden="true" />
                <span>{feature.label}</span>
              </div>
              <h3>{feature.title}</h3>
              <p>{feature.description}</p>
              <span className={styles.compactCardAction}>
                Open tool
                <ArrowRight size={15} aria-hidden="true" />
              </span>
            </Link>
          )
        })}
      </div>
    </section>

    <section className={styles.ctaSection} aria-labelledby="landing-cta-title">
      <div className={styles.ctaPanel}>
        <span className={styles.ctaIcon}>
          <Blocks size={26} strokeWidth={1.7} aria-hidden="true" />
        </span>
        <p className={styles.ctaEyebrow}>Start tracing</p>
        <h2 id="landing-cta-title">
          The chain already knows what happened.
          <br />
          <span className={styles.accentText}>Make it readable.</span>
        </h2>
        <p className={styles.ctaDescription}>
          Search any account, open a recent transaction, or rerun a message in the emulator.
        </p>
        <div className={styles.ctaActions}>
          <Link className={styles.primaryAction} to="/">
            Open the explorer
            <ArrowRight size={17} aria-hidden="true" />
          </Link>
          <a
            className={styles.secondaryAction}
            href="https://github.com/ton-blockchain/acton"
            target="_blank"
            rel="noreferrer"
          >
            <Github size={17} aria-hidden="true" />
            GitHub
          </a>
        </div>
      </div>
    </section>

    <footer className={styles.footer}>
      <span>actonscan</span>
      <span className={styles.footerDivider} aria-hidden="true" />
      <span>Developer tools for TON execution</span>
      <a href="https://t.me/toncore" target="_blank" rel="noreferrer">
        TON Core
      </a>
    </footer>
  </div>
)
