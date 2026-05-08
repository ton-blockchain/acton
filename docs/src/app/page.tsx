import {Metadata} from "next"
import Image from "next/image"
import Link from "next/link"
import {
  ArrowRight,
  BookOpen,
  Bot,
  Bug,
  Check,
  CodeXml,
  Dna,
  FileCode,
  FlaskConical,
  Gauge,
  GitFork,
  Github,
  Library,
  MonitorPlay,
  ScanSearch,
  Shuffle,
  Terminal,
  Wallet,
  Workflow,
} from "lucide-react"
import type {ComponentType, ReactNode, SVGProps} from "react"
import {HeaderSearchField} from "@/components/HeaderSearchField"
import {InlineInstallationCommand} from "@/components/InstallationCodeBlock"
import {LandingVideo} from "@/components/LandingVideo"
import logoDark from "@/public/logo-dark.svg"

const landingUrl = "https://ton-blockchain.github.io/acton"
const landingOgImage = `${landingUrl}/og/home/image.png`

export const metadata: Metadata = {
  title: "Acton — TON Development Toolkit",
  description:
    "Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.",
  metadataBase: new URL(landingUrl),
  openGraph: {
    title: "Acton — TON Development Toolkit",
    description:
      "Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.",
    url: landingUrl,
    images: landingOgImage,
    locale: "en_US",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "Acton — TON Development Toolkit",
    description:
      "Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.",
    images: landingOgImage,
  },
}

interface FeatureWithVideo {
  title: string
  highlight: string
  description: string
  description2: string
  icon: ComponentType<SVGProps<SVGSVGElement>>
  url: string
}

interface FeatureBelow {
  title: string
  description: string
  docLink: string
  label: string
  icon: ComponentType<SVGProps<SVGSVGElement>>
  className: string
  featured?: boolean
}

const VIDEO_FEATURES: FeatureWithVideo[] = [
  {
    title: "Native Tolk Tests",
    highlight: "Tolk",
    description:
      "Use Tolk itself for testing. Unit tests, transaction flows, and cross‑contract interaction — without switching languages.",
    description2: "50x faster than TypeScript + JS sandbox.",
    icon: FlaskConical,
    url: "https://cdn.tapps.ninja/tolk/final-1.mov",
  },
  {
    title: "dApp-ready contracts",
    highlight: "dApp-ready",
    description: "Generate TypeScript wrappers for frontend and end‑to‑end testing.",
    description2: "TON Connect and React as the UI — Tolk and TON Blockchain as the backend.",
    icon: CodeXml,
    url: "https://cdn.tapps.ninja/tolk/final-2.mov",
  },
  {
    title: "Friendly for AI agents",
    highlight: "AI agents",
    description:
      "Skills and manuals available out of the box. Acton is a modern CLI tool that becomes an agent's runtime.",
    description2: "Built for humans — perfect for AI.",
    icon: Bot,
    url: "https://cdn.tapps.ninja/tolk/final-3.mov",
  },
  {
    title: "Debugger, done right",
    highlight: "Debugger",
    description:
      "Test failed with exit code 9? Stop exactly at the exception, inspect the call stack, local variables, lazy fields, and more.",
    description2: "Works for fully‑optimized production contracts, in all IDEs.",
    icon: Bug,
    url: "https://cdn.tapps.ninja/tolk/final-4.mov",
  },
  {
    title: "Faucet and deployment",
    highlight: "Faucet",
    description:
      "Not only develop, but deploy, verify, and configure your contracts. Acton manages wallets and faucet top‑ups on testnet.",
    description2: "Arbitrary on‑chain scripts — just using Tolk.",
    icon: Wallet,
    url: "https://cdn.tapps.ninja/tolk/final-5.mov",
  },
  {
    title: "IDE integration",
    highlight: "IDE",
    description: "Linter and formatter to keep code style consistent. All rules are configurable.",
    description2: "VS Code, JetBrains, Cursor, Zed, and other LSP‑based editors.",
    icon: FileCode,
    url: "https://cdn.tapps.ninja/tolk/final-6.mov",
  },
  {
    title: "Test UI: visualize traces",
    highlight: "Test UI",
    description:
      "Inspect transaction trees, messages, fees, storage changes — for every test, in a clean dev‑oriented UI.",
    description2: "Raw binary data decoded based on Tolk ABI.",
    icon: MonitorPlay,
    url: "https://cdn.tapps.ninja/tolk/final-7.mov",
  },
]

const BELOW_FEATURES: FeatureBelow[] = [
  {
    title: "Mutation testing",
    description:
      "Stress the suite by flipping operators, values, and branches to reveal weak checks and untested invariants.",
    docLink: "/docs/testing/mutation-testing/overview",
    label: "Suite strength",
    icon: Dna,
    className: "",
    featured: true,
  },
  {
    title: "Coverage reports",
    description:
      "Track line and branch coverage, inspect colorful reports in Test UI, and export LCOV for external tooling.",
    docLink: "/docs/testing/code-coverage",
    label: "Reports",
    icon: ScanSearch,
    className: "",
    featured: true,
  },
  {
    title: "Fork testing",
    description:
      "Run tests against real mainnet state by automatically pulling deployed accounts into the local emulator.",
    docLink: "/docs/testing/fork-testing",
    label: "Mainnet state",
    icon: GitFork,
    className: "",
  },
  {
    title: "Gas profiling",
    description:
      "Snapshot transaction-chain gas usage, compare against baselines, and catch fee regressions before they ship.",
    docLink: "/docs/testing/gas-profiling-with-snapshots",
    label: "Fee regressions",
    icon: Gauge,
    className: "",
  },
  {
    title: "Fuzz testing",
    description:
      "Re-run parameterized tests with generated inputs, assumptions, bounds, and reproducible seeds until the first failure.",
    docLink: "/docs/testing/fuzz-testing",
    label: "Generated inputs",
    icon: Shuffle,
    className: "",
  },
  {
    title: "Scripting",
    description:
      "Use Tolk for local experiments, deployment flows, and real blockchain interaction with familiar testing primitives.",
    docLink: "/docs/scripting/overview",
    label: "Automation",
    icon: Terminal,
    className: "",
  },
  {
    title: "CI integration",
    description:
      "Wire Acton into GitHub Actions or GitLab CI for builds, tests, checks, coverage, and secret-backed jobs.",
    docLink: "/docs/ci-setup",
    label: "Pipelines",
    icon: Workflow,
    className: "",
  },
  {
    title: "Masterchain libraries",
    description:
      "Publish reusable code to the Masterchain, track storage runway, top it up, and refer to it in tests and scripts.",
    docLink: "/docs/libraries",
    label: "Reusable code",
    icon: Library,
    className: "",
  },
]

const HEADER_ICON_LINKS: {
  href: string
  label: string
  icon: ComponentType<SVGProps<SVGSVGElement>>
}[] = [
  {href: "/docs/welcome", label: "Documentation", icon: BookOpen},
  {href: "https://t.me/toncore", label: "TON Core Telegram", icon: TelegramIcon},
  {href: "https://github.com/ton-blockchain/acton", label: "GitHub", icon: Github},
]

export default function Home() {
  return <ReleaseAnnouncement />
}

function ReleaseAnnouncement() {
  return (
    <main className="min-h-screen overflow-hidden bg-[#0f1115] text-[#f8f8f4]">
      <section className="relative flex min-h-screen items-center justify-center px-5 py-12 sm:px-8">
        <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(255,255,255,0.045)_1px,transparent_1px),linear-gradient(180deg,rgba(255,255,255,0.04)_1px,transparent_1px)] bg-[size:28px_28px]" />
        <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-[#9AE7FF]/70 to-transparent" />
        <div className="absolute inset-x-0 bottom-0 h-px bg-gradient-to-r from-transparent via-[#2D83EC]/70 to-transparent" />

        <div className="relative w-full max-w-5xl">
          <div className="mb-8 flex items-center justify-between gap-4">
            <div className="flex items-center gap-3">
              <Image
                src={logoDark}
                alt="Acton logo"
                width={36}
                height={36}
                className="h-9 w-9 rounded-md"
                priority
              />
              <span className="text-lg font-semibold tracking-tight text-white">Acton</span>
            </div>
            <span className="rounded-full border border-[#9AE7FF]/30 bg-[#9AE7FF]/10 px-3 py-1 text-xs font-semibold uppercase tracking-[0.18em] text-[#9AE7FF]">
              May 11
            </span>
          </div>

          <div className="border-y border-white/12 py-12 sm:py-16 md:py-20">
            <p className="text-sm font-semibold uppercase tracking-[0.22em] text-[#9AE7FF]/80">
              Release is coming
            </p>
            <h1 className="mt-5 max-w-4xl text-5xl font-semibold leading-[0.98] text-white sm:text-7xl md:text-[6.5rem]">
              Launching on
              <br />
              May 11
            </h1>
          </div>
        </div>
      </section>
    </main>
  )
}

export function LandingHome() {
  return (
    <div className="home-shell min-h-screen overflow-x-hidden bg-[#121212] text-[#f7f7f2]">
      <SiteHeader />
      <main>
        <section className="mx-auto max-w-[1400px] px-3 pt-8 sm:px-4 sm:pt-14 md:px-8 lg:px-10">
          <div className="home-hero-panel relative overflow-hidden border-x border-t border-white/10">
            <div className="relative z-10 px-10 pb-[21rem] pt-10 sm:px-8 sm:pb-[28rem] sm:pt-14 lg:px-12 lg:pb-28 lg:pt-16 xl:pb-32">
              <div className="mx-auto w-full">
                <div className="max-w-[58rem]">
                  <Link
                    href="https://t.me/durov/501"
                    target="_blank"
                    className="group mb-8 inline-flex w-fit items-center gap-2 rounded-full border border-[#1AC9FF]/45 bg-white/[0.03] px-3 py-1.5 text-xs font-medium text-[#9AE7FF] transition-colors hover:border-[#9AE7FF]/70 hover:bg-[#9AE7FF]/10 hover:text-[#c9f4ff]"
                  >
                    MTONGA plan
                    <ArrowRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
                  </Link>
                  <h1 className="text-[2.3rem] font-semibold leading-[1.08] text-[#f8f8f4] sm:text-[2.8rem] sm:leading-[1.04] lg:text-[3.4rem] xl:text-[4rem]">
                    A unified toolchain for{" "}
                    <span className="text-[#9AE7FF]">TON&nbsp;smart&nbsp;contracts</span>
                  </h1>
                  <p className="mt-6 max-w-2xl text-lg leading-7 text-[#c7c6bf] sm:mt-8 sm:text-xl sm:leading-8">
                    <span className="font-semibold text-[#9AE7FF]">Acton</span> is an all-in-one CLI
                    built around <span className="font-semibold text-[#9AE7FF]">Tolk</span>
                    &nbsp;—&nbsp;from project creation to tests, debugging, dApp&nbsp;integration,
                    deployment, and verification.
                  </p>
                  <p className="mt-6 max-w-2xl text-lg leading-7 text-[#c7c6bf] sm:mt-8 sm:text-xl sm:leading-8">
                    Built for humans.{" "}
                    <span className="font-semibold text-[#9AE7FF]">Perfect for AI.</span>
                  </p>

                  <div className="mt-10 space-y-4 sm:mt-14">
                    <p className="text-xs font-semibold uppercase tracking-[0.10em] text-[#8b8a82]">
                      Installation
                    </p>
                    <div className="w-full max-w-[30rem] min-w-0 sm:w-fit">
                      <InlineInstallationCommand />
                    </div>
                  </div>
                </div>
              </div>
            </div>
            <div className="pointer-events-none absolute -bottom-6 left-4 right-[-10rem] z-0 sm:-bottom-10 sm:left-8 sm:right-[-12rem] lg:bottom-[-4.5rem] lg:left-auto lg:right-[-5.5rem] lg:w-[62%] xl:right-[-2rem] xl:w-[60%]">
              <Image
                src="/landing/hero-ide.png"
                alt=""
                width={1436}
                height={1096}
                className="h-auto w-full opacity-60"
                priority
              />
              <div
                className="pointer-events-none absolute inset-y-0 left-10 w-50 bg-gradient-to-r from-[#121212] to-transparent"
                aria-hidden="true"
              />
            </div>
            <div
              className="pointer-events-none absolute inset-x-0 top-0 h-40 bg-gradient-to-b from-[#121212] to-transparent"
              aria-hidden="true"
            />
            <div
              className="pointer-events-none absolute inset-x-0 -bottom-5 h-40 bg-gradient-to-b from-transparent to-[#121212]"
              aria-hidden="true"
            />
          </div>
        </section>

        <section className="mx-auto max-w-[1400px] px-3 sm:px-4 md:px-8 lg:px-10">
          <div className="border-x border-white/10 bg-fd-background px-4 py-14 sm:px-8 sm:py-20 lg:px-12">
            <SectionHeader
              eyebrow="Essential features"
              title={
                <>
                  The <span className="text-[#9AE7FF]">on-chain workflow</span> reimagined
                </>
              }
              description="A full development environment built as one coherent system"
            />

            <div className="mt-8 space-y-14 sm:mt-24 sm:space-y-16 lg:space-y-20">
              {VIDEO_FEATURES.map((feature, index) => {
                const reversed = index % 2 === 1
                const Icon = feature.icon
                const highlightIndex = feature.title.indexOf(feature.highlight)

                return (
                  <article
                    key={feature.title}
                    className={`feature-video-card ${
                      reversed
                        ? "feature-video-card--video-left"
                        : "feature-video-card--video-right"
                    } grid overflow-hidden bg-fd-background ${
                      reversed ? "lg:grid-cols-[60%_40%]" : "lg:grid-cols-[40%_60%]"
                    }`}
                  >
                    <div className={`feature-copy-panel ${reversed ? "lg:order-2" : ""}`}>
                      <div
                        className={`flex h-full flex-col justify-start pt-12 sm:pt-14 lg:min-h-[360px] lg:pt-14 xl:min-h-[440px] xl:pt-20 ${
                          reversed
                            ? "pl-5 pr-10 lg:pl-7 lg:pr-12 xl:pr-14"
                            : "pl-10 pr-5 lg:pl-12 lg:pr-7 xl:pl-14"
                        }`}
                      >
                        <div
                          className={
                            reversed
                              ? "max-w-[25rem] lg:ml-auto lg:mr-0 lg:text-right"
                              : "max-w-[25rem] lg:ml-0 lg:mr-auto"
                          }
                        >
                          <div className={`mb-5 flex ${reversed ? "lg:justify-end" : ""}`}>
                            <span className="inline-flex text-[#9AE7FF]">
                              <Icon className="h-9 w-9" strokeWidth={1.8} />
                            </span>
                          </div>
                          <h3
                            className={`text-3xl font-semibold leading-tight text-[#f4f4ef] sm:text-[2.15rem] lg:whitespace-nowrap xl:text-[2.6rem] ${
                              reversed ? "lg:-ml-16 xl:-ml-24" : ""
                            }`}
                          >
                            {feature.title.slice(0, highlightIndex)}
                            <span className="text-[#9AE7FF]">{feature.highlight}</span>
                            {feature.title.slice(highlightIndex + feature.highlight.length)}
                          </h3>
                          <p className="mt-4 text-base leading-5 text-[#c7c6bf] xl:mt-5 xl:text-lg xl:leading-6">
                            {feature.description}
                          </p>
                          <p className="mt-3 text-base leading-5 text-[#c7c6bf] xl:mt-4 xl:text-lg xl:leading-6">
                            {feature.description2}
                          </p>
                        </div>
                      </div>
                    </div>

                    <div className={reversed ? "lg:order-1" : ""}>
                      <div className="feature-video-panel min-h-[240px] sm:min-h-[340px] lg:min-h-[360px] xl:min-h-[440px]">
                        <LandingVideo
                          className={`feature-video ${
                            reversed ? "feature-video--align-left" : "feature-video--align-right"
                          }`}
                          controls
                          controlsList="nodownload noplaybackrate"
                          playLabel={`Play ${feature.title} video`}
                          playsInline
                          preload="metadata"
                          src={feature.url}
                        >
                          Your browser does not support the video tag.
                        </LandingVideo>
                      </div>
                    </div>
                  </article>
                )
              })}
            </div>

        <section className="mx-auto mb-16 max-w-[1400px] px-3 sm:mb-20 sm:px-4 md:px-8 lg:px-10">
          <div className="border-x border-b border-white/10 bg-fd-background px-4 py-14 sm:px-8 sm:py-20 lg:px-12">
            <SectionHeader
              eyebrow="Extra depth"
              title={
                <>
                  <span className="text-[#9AE7FF]">More</span> than the happy path
                </>
              }
              description="Deep dive into security, error prevention, and gas optimization"
            />

            <div className="mx-auto mt-12 grid max-w-6xl gap-4 sm:mt-16 md:grid-cols-2">
              {BELOW_FEATURES.map(feature => {
                const Icon = feature.icon

                return (
                  <Link
                    key={feature.title}
                    href={feature.docLink}
                    className={`group relative min-h-[220px] overflow-hidden rounded-lg border border-white/10 bg-white/[0.025] p-5 text-left transition duration-200 hover:border-[#9AE7FF]/35 hover:bg-white/[0.04] sm:p-6 ${
                      feature.featured ? "sm:min-h-[250px]" : "sm:min-h-[230px]"
                    } ${feature.className}`}
                  >
                    <span
                      className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_20%_0%,rgba(154,231,255,0.12),transparent_34%)] opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                      aria-hidden="true"
                    />
                    <div className="relative flex h-full flex-col">
                      <div className="mb-4 flex items-start justify-between gap-4">
                        <span className="inline-flex shrink-0 items-center justify-start text-[#9AE7FF]">
                          <Icon
                            className={feature.featured ? "h-6 w-6" : "h-5 w-5"}
                            strokeWidth={1.8}
                          />
                        </span>
                        <span className="pt-1 text-right text-[0.68rem] font-semibold uppercase tracking-[0.12em] text-[#65645f]">
                          {feature.label}
                        </span>
                      </div>

                      <h3
                        className={`font-semibold leading-tight text-[#f4f4ef] ${
                          feature.featured ? "text-3xl sm:text-[2.1rem]" : "text-2xl"
                        }`}
                      >
                        {feature.title}
                      </h3>
                      <p
                        className={`mt-4 text-[#b9b8b1] ${
                          feature.featured ? "max-w-2xl text-base leading-7" : "text-base leading-6"
                        }`}
                      >
                        {feature.description}
                      </p>
                      <span className="mt-auto inline-flex items-center gap-2 pt-8 text-sm font-medium text-[#9AE7FF]/65 transition-colors group-hover:text-[#9AE7FF]">
                        Read in docs
                        <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" />
                      </span>
                    </div>
                  </Link>
                )
              })}
            </div>
          </div>
        </section>

            <div className="mx-auto mt-20 max-w-6xl sm:mt-28">
              <p className="flex items-center gap-4 text-xs font-semibold uppercase tracking-[0.10em] text-[#8b8a82]">
                <span className="h-px flex-1 bg-white/10" />
                <span className="shrink-0 px-2">Migration path</span>
                <span className="h-px flex-1 bg-white/10" />
              </p>
              <div className="pt-10 sm:pt-12 lg:pt-16">
                <div className="grid gap-10 lg:grid-cols-[0.78fr_1.22fr] lg:items-center xl:gap-12">
                  <div className="max-w-lg">
                    <h2 className="text-3xl font-semibold leading-[1.04] text-[#f8f8f4] sm:text-5xl">
                      Migrate <span className="text-[#9AE7FF]">FunC</span> contracts{" "}
                      <span className="text-[#9AE7FF]">to Tolk</span>
                    </h2>
                    <p className="mt-5 text-base leading-6 text-[#c7c6bf] sm:mt-6 sm:text-lg sm:leading-7">
                      Give an agent your existing FunC + Blueprint project, and it will handle the migration end-to-end.
                    </p>
                    <p className="mt-5 text-base leading-6 text-[#c7c6bf] sm:mt-6 sm:text-lg sm:leading-7">
                      Available via func2tolk <span className="text-[#9AE7FF]">skill</span> for Codex/Claude.
                    </p>

                    <div className="mt-8 flex flex-wrap items-center gap-x-4 gap-y-3 sm:mt-10">
                      <Link
                        href="/docs/commands/func2tolk"
                        className="inline-flex h-11 items-center gap-2 rounded-lg border border-white/12 bg-white/[0.03] px-5 text-sm font-medium text-[#deddd5] transition-colors hover:bg-white/[0.06]"
                      >
                        Migrate a FunC project
                        <ArrowRight className="h-4 w-4" />
                      </Link>
                    </div>
                  </div>

                  <div className="relative lg:-translate-y-5 xl:-translate-y-5">
                    <div className="mx-auto max-w-[720px]">
                      <Image
                        src="/landing/func2tolk-migration.png"
                        alt="FunC to Tolk migration diagram"
                        width={1365}
                        height={766}
                        className="h-auto w-full opacity-75"
                      />
                    </div>

                    <div className="mx-auto mt-4 grid max-w-[720px] gap-x-6 gap-y-3 sm:grid-cols-2 lg:translate-x-6 xl:translate-x-8">
                      <div className="flex items-center gap-3 text-sm font-medium leading-5 text-[#aaa9a2] lg:pl-8 xl:pl-12">
                        <Check className="h-4 w-4 shrink-0 text-emerald-300/75" />
                        <span>Contracts become idiomatic Tolk</span>
                      </div>
                      <div className="flex items-center gap-3 text-sm font-medium leading-5 text-[#aaa9a2]">
                        <Check className="h-4 w-4 shrink-0 text-emerald-300/75" />
                        <span>Fast native code instead of TypeScript</span>
                      </div>
                      <div className="flex items-center gap-3 text-sm font-medium leading-5 text-[#aaa9a2] lg:pl-8 xl:pl-12">
                        <Check className="h-4 w-4 shrink-0 text-emerald-300/75" />
                        <span>Gas consumption goes down</span>
                      </div>
                      <div className="flex items-center gap-3 text-sm font-medium leading-5 text-[#aaa9a2]">
                        <Check className="h-4 w-4 shrink-0 text-emerald-300/75" />
                        <span>Continue developing with green tests</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <div className="mx-auto mt-20 max-w-6xl sm:mt-28">
              <SectionHeader
                eyebrow="Start building"
                title="Ready to build?"
                description="Install Acton and start from the docs."
              />
              <div className="mt-10 flex flex-col items-center gap-8 text-center sm:mt-12 sm:gap-10">
                <div className="w-full max-w-[30rem] min-w-0 sm:w-fit">
                  <InlineInstallationCommand />
                </div>
                <div className="flex flex-wrap justify-center gap-3">
                  <Link
                    href="/docs/welcome"
                    className="inline-flex h-11 items-center gap-2 rounded-lg bg-white px-5 text-sm font-medium text-black transition-colors hover:bg-white/90"
                  >
                    Get Started
                    <ArrowRight className="h-4 w-4" />
                  </Link>
                  <Link
                    href="https://github.com/ton-blockchain/acton"
                    target="_blank"
                    className="inline-flex h-11 items-center gap-2 rounded-lg border border-white/12 bg-white/[0.03] px-5 text-sm font-medium text-[#deddd5] transition-colors hover:bg-white/[0.06]"
                  >
                    <Github className="h-4 w-4" />
                    GitHub
                  </Link>
                </div>
              </div>
            </div>
          </div>
        </section>
      </main>

      <footer className="border-t border-white/10 bg-[#121212]/90">
        <div className="mx-auto flex h-[55px] max-w-[1400px] items-center justify-between px-4 md:h-16 md:px-8 lg:px-10">
          <div className="inline-flex flex-wrap items-baseline gap-x-2 gap-y-1 leading-none">
            <Link
              href="/"
              className="text-xl font-semibold leading-none text-white transition-colors hover:text-[#f8f8f4]"
            >
              Acton
            </Link>
            <span className="text-sm font-medium leading-none text-[#c7c6bf]">by</span>
            <Link
              href="https://t.me/toncore"
              target="_blank"
              className="text-sm font-medium leading-none text-[#9AE7FF] transition-colors hover:text-[#1AC9FF]"
            >
              TON Core
            </Link>
          </div>
          <div className="flex items-center gap-1">
            {HEADER_ICON_LINKS.map(({href, label, icon: Icon}) => (
              <IconLink key={label} href={href} label={label} icon={Icon} />
            ))}
          </div>
        </div>
      </footer>
    </div>
  )
}

function SiteHeader() {
  return (
    <header className="sticky top-0 z-40 border-b border-white/10 bg-[#121212]/90 backdrop-blur-xl">
      <nav className="mx-auto flex h-[55px] max-w-[1400px] items-center justify-between px-4 md:h-16 md:px-8 lg:px-10">
        <div className="flex items-center gap-5">
          <Link href="/" className="flex items-center gap-3 md:-translate-x-px">
            <Image
              src={logoDark}
              alt="Acton logo"
              width={32}
              height={32}
              className="h-8 w-8 rounded-md"
              priority
            />
            <span className="text-lg font-semibold tracking-tight text-white">Acton</span>
          </Link>
          <span className="hidden h-5 w-px bg-white/14 sm:block" />
          <div className="hidden items-center gap-6 sm:flex">
            <Link
              href="/docs/welcome"
              className="text-sm text-[#c9c8c0] transition-colors hover:text-white"
            >
              Docs
            </Link>
            <Link
              href="/docs/commands/overview"
              className="text-sm text-[#c9c8c0] transition-colors hover:text-white"
            >
              Commands
            </Link>
            <Link
              href="/docs/testing/overview"
              className="text-sm text-[#c9c8c0] transition-colors hover:text-white"
            >
              Testing
            </Link>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1">
            <HeaderSearchField />
            {HEADER_ICON_LINKS.map(({href, label, icon: Icon}) => (
              <IconLink key={label} href={href} label={label} icon={Icon} />
            ))}
          </div>
        </div>
      </nav>
    </header>
  )
}

function IconLink({
  href,
  label,
  icon: Icon,
}: {
  href: string
  label: string
  icon: ComponentType<SVGProps<SVGSVGElement>>
}) {
  return (
    <Link
      href={href}
      target={href.startsWith("http") ? "_blank" : undefined}
      aria-label={label}
      className="inline-flex h-9 w-9 items-center justify-center rounded-lg text-[#c9c8c0] transition-colors hover:bg-white/[0.06] hover:text-white"
    >
      <Icon className="h-4 w-4" />
    </Link>
  )
}

function TelegramIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        d="M20.86 4.68 17.7 19.56c-.24 1.05-.86 1.3-1.74.8l-4.82-3.55-2.32 2.24c-.26.26-.47.47-.97.47l.34-4.91 8.94-8.08c.39-.34-.08-.53-.6-.19L5.49 13.29.73 11.8c-1.03-.32-1.05-1.03.22-1.53L19.56 3.1c.86-.32 1.61.19 1.3 1.58Z"
        fill="currentColor"
      />
    </svg>
  )
}

function SectionHeader({
  eyebrow,
  title,
  description,
}: {
  eyebrow: string
  title: ReactNode
  description: string
}) {
  return (
    <div className="text-center max-w-5xl mx-auto">
      <p className="flex items-center gap-4 text-xs font-semibold uppercase tracking-[0.10em] text-[#8b8a82]">
        <span className="h-px flex-1 bg-white/10" aria-hidden="true" />
        <span className="shrink-0 px-2">{eyebrow}</span>
        <span className="h-px flex-1 bg-white/10" aria-hidden="true" />
      </p>
      <div className="mx-auto max-w-3xl">
        <h2 className="mt-12 text-3xl font-semibold leading-tight text-[#f8f8f4] sm:text-5xl">
          {title}
        </h2>
        <p className="mt-4 text-lg leading-7 text-[#a0a0a0] sm:mt-4 sm:text-xl sm:leading-8 font-light">
          {description}
        </p>
      </div>
    </div>
  )
}
