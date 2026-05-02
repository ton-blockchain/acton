import { Metadata } from 'next';
import Image from 'next/image';
import Link from 'next/link';
import {
  ArrowRight,
  BookOpen,
  Bot,
  Bug,
  CodeXml,
  FileCode,
  FlaskConical,
  Github,
  MonitorPlay,
  Wallet,
} from 'lucide-react';
import type { ComponentType, SVGProps } from 'react';
import { InlineInstallationCommand } from '@/components/InstallationCodeBlock';
import { LandingVideo } from '@/components/LandingVideo';
import logoDark from '@/public/logo-dark.svg';

export const metadata: Metadata = {
  title: 'Acton — TON Development Toolkit',
  description: 'Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.',
  metadataBase: new URL('https://ton-blockchain.github.io/acton'),
  openGraph: {
    title: 'Acton — TON Development Toolkit',
    description: 'Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.',
    url: 'https://ton-blockchain.github.io/acton',
    images: '/og/home',
    locale: 'en_US',
    type: 'website',
  },
  twitter: {
    card: 'summary_large_image',
    title: 'Acton — TON Development Toolkit',
    description: 'Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.',
    images: '/og/home',
  },
};

interface FeatureWithVideo {
  title: string;
  description: string;
  description2: string;
  url: string;
}

interface FeatureBelow {
  title: string;
  description: string;
  docLink: string;
}

const VIDEO_FEATURES: FeatureWithVideo[] = [
  {
    title: 'Native Tolk Tests',
    description: 'Use Tolk itself for testing. Unit tests, transaction flows, and cross-contract interaction — without switching languages.',
    description2: '50x faster than TypeScript + JS sandbox.',
    url: 'https://cdn.tapps.ninja/tolk/final-1.mov',
  },
  {
    title: 'dApp-ready contracts',
    description: 'Generate TypeScript wrappers for frontend and end-to-end testing.',
    description2: 'TON Connect and AppKit as the UI — Tolk and TON Blockchain as the backend.',
    url: 'https://cdn.tapps.ninja/tolk/final-2.mov',
  },
  {
    title: 'Friendly for AI agents',
    description: 'Skills and manuals available out of the box. Acton is a modern CLI tool that becomes an agent\'s runtime.',
    description2: 'Built for humans — perfect for AI.',
    url: 'https://cdn.tapps.ninja/tolk/final-3.mov',
  },
  {
    title: 'Debugger, done right',
    description: 'Test failed with exit code 9? Stop exactly at the exception, inspect the call stack, local variables, lazy fields, and more.',
    description2: 'Works for fully-optimized production contracts, in all IDEs.',
    url: 'https://cdn.tapps.ninja/tolk/final-4.mov',
  },
  {
    title: 'Faucet and deployment',
    description: 'Not only develop, but deploy, verify, and configure your contracts. Acton manages wallets and faucet top-ups on testnet.',
    description2: 'Arbitrary on-chain scripts — just using Tolk.',
    url: 'https://cdn.tapps.ninja/tolk/final-5.mov',
  },
  {
    title: 'IDE integration',
    description: 'Linter and formatter to keep code style consistent. All rules are configurable.',
    description2: 'VS Code, JetBrains, Cursor, Helix, Zed, (Neo)Vim, and other LSP-based editors.',
    url: 'https://cdn.tapps.ninja/tolk/final-6.mov',
  },
  {
    title: 'Test UI: visualize traces',
    description: 'Inspect transaction trees, messages, fees, storage changes — for every test, in a clean dev-oriented UI.',
    description2: 'Raw binary data decoded based on Tolk ABI.',
    url: 'https://cdn.tapps.ninja/tolk/final-7.mov',
  },
];

const BELOW_FEATURES: FeatureBelow[] = [
  {
    title: 'Mutation testing',
    description: 'Stress the suite by flipping operators, values, and branches to reveal weak checks and untested invariants.',
    docLink: '/docs/testing/mutation-testing/overview',
  },
  {
    title: 'Coverage reports',
    description: 'Track line and branch coverage, inspect colorful reports in Test UI, and export LCOV for external tooling.',
    docLink: '/docs/testing/code-coverage',
  },
  {
    title: 'Fork testing',
    description: 'Run tests against real mainnet state by automatically pulling deployed accounts into the local emulator.',
    docLink: '/docs/testing/fork-testing',
  },
  {
    title: 'Fuzz testing',
    description: 'Re-run parameterized tests with generated inputs, assumptions, bounds, and reproducible seeds until the first failure.',
    docLink: '/docs/testing/fuzz-testing',
  },
  {
    title: 'Gas profiling',
    description: 'Snapshot transaction-chain gas usage, compare against baselines, and catch fee regressions before they ship.',
    docLink: '/docs/testing/gas-profiling-with-snapshots',
  },
  {
    title: 'Scripting',
    description: 'Use Tolk for local experiments, deployment flows, and real blockchain interaction with familiar testing primitives.',
    docLink: '/docs/scripting/overview',
  },
  {
    title: 'CI integration',
    description: 'Wire Acton into GitHub Actions or GitLab CI for builds, tests, checks, coverage, and secret-backed jobs.',
    docLink: '/docs/ci-setup',
  },
  {
    title: 'Masterchain libraries',
    description: 'Publish reusable code to the Masterchain, track storage runway, top it up, and refer to it in tests and scripts.',
    docLink: '/docs/libraries',
  },
];

const FEATURE_TITLE_ACCENTS: Record<string, string> = {
  'Native Tolk Tests': 'Tolk',
  'dApp-ready contracts': 'dApp-ready',
  'Friendly for AI agents': 'AI',
  'Debugger, done right': 'Debugger',
  'Faucet and deployment': 'Faucet',
  'IDE integration': 'IDE',
  'Test UI: visualize traces': 'Test UI',
};

const FEATURE_ICONS: Record<string, {
  icon: ComponentType<SVGProps<SVGSVGElement>>;
}> = {
  'Native Tolk Tests': {
    icon: FlaskConical,
  },
  'dApp-ready contracts': {
    icon: CodeXml,
  },
  'Friendly for AI agents': {
    icon: Bot,
  },
  'Debugger, done right': {
    icon: Bug,
  },
  'Faucet and deployment': {
    icon: Wallet,
  },
  'IDE integration': {
    icon: FileCode,
  },
  'Test UI: visualize traces': {
    icon: MonitorPlay,
  },
};

const HEADER_ICON_LINKS: {
  href: string;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
}[] = [
  { href: '/docs/welcome', label: 'Documentation', icon: BookOpen },
  { href: 'https://t.me/toncore', label: 'TON Core Telegram', icon: TelegramIcon },
  { href: 'https://github.com/ton-blockchain/acton', label: 'GitHub', icon: Github },
];

export default function Home() {
  return (
    <div className="home-shell min-h-screen overflow-x-hidden bg-[#050505] text-[#f7f7f2]">
      <SiteHeader />
      <main>
        <section className="mx-auto max-w-[1500px] px-3 pt-8 sm:px-4 sm:pt-14 md:px-8 lg:px-10">
          <div className="border-x border-t border-white/10 bg-[#070707]/92">
            <div className="px-4 py-10 sm:px-8 sm:pb-12 sm:pt-14 lg:px-12 lg:pb-14 lg:pt-16">
                <Link
                  href="/docs/welcome"
                  className="mb-8 inline-flex w-fit items-center gap-2 rounded-full border border-[#1AC9FF]/45 bg-[#2D83EC]/10 px-3 py-1.5 text-xs font-medium text-[#9AE7FF] transition-colors hover:bg-[#2D83EC]/15"
                >
                  <span className="h-1.5 w-1.5 rounded-full bg-[#1AC9FF]" />
                  Acton 1.0 and Tolk 1.4 is here
                  <ArrowRight className="h-3.5 w-3.5" />
                </Link>

                <h1 className="max-w-5xl text-[2.3rem] font-semibold leading-[1.08] text-[#f8f8f4] sm:text-6xl sm:leading-[1.04] lg:text-[4.9rem] xl:text-[5.8rem]">
                  Everything You Need to Build Contracts on <span className="text-[#9AE7FF]">TON</span>
                </h1>
                <p className="mt-6 max-w-2xl text-lg leading-7 text-[#c7c6bf] sm:mt-8 sm:text-xl sm:leading-8">
                  <span className="font-semibold text-[#9AE7FF]">Acton</span> is an all-in-one TON smart contract development toolkit with blazingly fast feedback loops for
                  building, testing, debugging, scripting, deployment, and many more.
                </p>

                <div className="mt-8 space-y-4 sm:mt-10">
                  <p className="text-xs font-semibold uppercase text-[#8b8a82]">
                    Get started in 30 seconds
                  </p>
                  <div className="w-full max-w-[30rem] min-w-0 sm:w-fit">
                    <InlineInstallationCommand />
                  </div>
                </div>
            </div>

          </div>
        </section>

        <section className="mx-auto max-w-[1500px] px-3 sm:px-4 md:px-8 lg:px-10">
          <div className="border-x border-b border-white/10 bg-[#070707] px-4 py-14 sm:px-8 sm:py-20 lg:px-12">
            <div className="max-w-3xl">
              <p className="text-xs font-semibold uppercase tracking-[0.18em] text-[#9AE7FF]/70">
                From zero to testnet
              </p>
              <h2 className="mt-4 text-3xl font-semibold leading-tight text-[#f8f8f4] sm:text-5xl">
                2-minute walkthrough
              </h2>
              <p className="mt-5 text-lg leading-7 text-[#c7c6bf] sm:mt-6 sm:text-xl sm:leading-8">
                Tiny Acton workshop: create a project, launch tests, guide through essential features,
                airdrop TONs to your wallet, and deploy a contract to TON Blockchain.
              </p>
            </div>

            <div className="mt-8 overflow-hidden rounded-[1.25rem] border border-white/10 bg-[#0c0c0c] shadow-[0_24px_80px_rgba(0,0,0,0.28)] sm:mt-10 sm:rounded-[1.75rem]">
              <div className="flex aspect-video items-center justify-center border-b border-white/8 bg-[linear-gradient(90deg,rgba(255,255,255,0.04)_1px,transparent_1px),linear-gradient(180deg,rgba(255,255,255,0.04)_1px,transparent_1px)] bg-[size:24px_24px] px-4 text-center sm:px-8">
                <div className="max-w-2xl">
                  <p className="text-xs font-semibold uppercase tracking-[0.18em] text-[#77766f]">
                    Placeholder
                  </p>
                  <h3 className="mt-4 text-2xl font-semibold leading-tight text-[#f4f4ef] md:text-5xl">
                    I will record this video the next week
                  </h3>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section className="mx-auto max-w-[1500px] px-3 sm:px-4 md:px-8 lg:px-10">
          <div className="border-x border-b border-white/10 bg-[#070707] px-4 py-14 sm:px-8 sm:py-20 lg:px-12">
            <SectionHeader
              eyebrow="Essential features"
              title="A unified toolchain for TON"
              description="Not another small utility. Not another isolated CLI. A full development environment built as one coherent system around Tolk."
            />

            <div className="mt-8 space-y-10 sm:mt-14">
              {VIDEO_FEATURES.map((feature, index) => {
                const reversed = index % 2 === 1;

                return (
                  <article
                    key={feature.title}
                    className="grid overflow-hidden rounded-[1.25rem] border border-white/10 bg-[#070707] sm:rounded-[1.75rem] lg:grid-cols-2"
                  >
                    <div
                      className={`flex flex-col justify-start p-5 py-10 sm:p-8 lg:min-h-[440px] lg:p-10 ${
                        reversed ? 'lg:order-2' : ''
                      }`}
                    >
                      <div className={reversed ? 'lg:ml-auto lg:mt-10 lg:max-w-xl lg:text-right' : 'max-w-xl lg:mt-10'}>
                        <div className={`mb-5 flex ${reversed ? 'lg:justify-end' : ''}`}>
                          <FeatureIcon title={feature.title} />
                        </div>
                        <h3 className="text-3xl font-semibold leading-tight text-[#f4f4ef] sm:text-[2.75rem]">
                          <FeatureTitle title={feature.title} />
                        </h3>
                        <p className="mt-6 text-base leading-7 text-[#c7c6bf] sm:text-lg sm:leading-8">
                          {feature.description}
                        </p>
                        <p className="mt-5 text-base leading-7 text-[#c7c6bf] sm:text-lg sm:leading-8">
                          {feature.description2}
                        </p>
                      </div>
                    </div>

                    <div className={reversed ? 'lg:order-1' : ''}>
                      <div className="feature-video-panel min-h-[240px] sm:min-h-[380px] lg:min-h-[440px]">
                        <LandingVideo
                          className="feature-video"
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
                );
              })}
            </div>
          </div>
        </section>

        <section className="mx-auto max-w-[1500px] px-3 sm:px-4 md:px-8 lg:px-10">
          <div className="border-x border-white/10 bg-[#070707] px-4 py-14 sm:px-8 sm:py-20 lg:px-12">
            <SectionHeader
              eyebrow="Extra depth"
              title="More than the happy path"
              description="Several principle features that allow dive deep into contract development. Focused on security, error prevention, and gas optimization."
            />

            <div className="mt-8 grid gap-px overflow-hidden rounded-[1.25rem] border border-white/10 bg-white/10 sm:mt-10 sm:rounded-[1.75rem] md:grid-cols-2 xl:grid-cols-4">
              {BELOW_FEATURES.map((feature) => (
                <Link
                  key={feature.title}
                  href={feature.docLink}
                  className="group min-h-[220px] bg-[#0b0b0b] p-5 text-left transition-colors hover:bg-[#111110] sm:min-h-[280px] sm:p-7"
                >
                  <div className="flex h-full flex-col">
                    <h3 className="text-2xl font-semibold leading-tight text-[#f4f4ef]">
                      {feature.title}
                    </h3>
                    <p className="mt-5 text-base leading-7 text-[#c7c6bf]">
                      {feature.description}
                    </p>
                    <span className="mt-auto inline-flex items-center gap-2 pt-8 text-sm font-medium text-[#9AE7FF]">
                      Read in docs
                      <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" />
                    </span>
                  </div>
                </Link>
              ))}
            </div>
          </div>
        </section>

        <section className="mx-auto mt-4 max-w-[1500px] px-3 sm:px-4 md:px-8 lg:px-10">
          <div className="border border-white/10 bg-[#070707] px-4 pb-20 pt-16 sm:px-8 sm:pb-24 sm:pt-18 lg:px-12 lg:pb-28 lg:pt-20">
            <div className="grid gap-8 sm:gap-10 lg:grid-cols-[0.8fr_1.2fr] lg:items-center">
              <div>
                <p className="text-xs font-semibold uppercase tracking-[0.18em] text-[#9AE7FF]/70">
                  FunC migration
                </p>
                <h2 className="mt-4 text-3xl font-semibold leading-tight text-[#f8f8f4] sm:text-5xl">
                  FunC to Tolk in one click
                </h2>
                <p className="mt-5 max-w-xl text-base leading-7 text-[#c7c6bf] sm:mt-6 sm:text-lg sm:leading-8">
                  Convert your existing FunC project to Tolk with a single command,
                  and then iteratively refactor it keeping green tests.
                </p>
              </div>
              <div className="overflow-hidden rounded-xl border border-white/10 bg-[#050505] shadow-[0_24px_80px_rgba(0,0,0,0.3)] lg:translate-y-6">
                <div className="flex items-center justify-between border-b border-white/10 bg-white/[0.03] px-4 py-3 sm:px-5">
                  <div className="flex items-center gap-2.5">
                    <span className="h-3 w-3 rounded-full bg-[#ff6b6b]" />
                    <span className="h-3 w-3 rounded-full bg-[#ffd166]" />
                    <span className="h-3 w-3 rounded-full bg-[#06d6a0]" />
                  </div>
                </div>
                <div className="p-4 sm:p-5">
                  <pre className="overflow-x-auto font-mono text-sm leading-7">
                    <code>
                      <span className="text-[#77766f]">$ </span>
                      <span className="text-[#9AE7FF]">acton</span>
                      <span className="text-sky-200"> func2tolk </span>
                      <span className="text-[#ef8cff]">contracts/</span>
                      {'\n'}
                      <span className="text-[#1AC9FF]">✓</span>
                      <span className="text-[#d8d7cf]"> parsed </span>
                      <span className="text-sky-200">14</span>
                      <span className="text-[#d8d7cf]"> FunC files</span>
                      {'\n'}
                      <span className="text-[#1AC9FF]">✓</span>
                      <span className="text-[#d8d7cf]"> generated </span>
                      <span className="text-sky-200">Tolk</span>
                      <span className="text-[#d8d7cf]"> contracts</span>
                      {'\n'}
                      <span className="text-[#1AC9FF]">✓</span>
                      <span className="text-[#d8d7cf]"> wrappers are still compatible</span>
                      {'\n'}
                      <span className="text-[#1AC9FF]">✓</span>
                      <span className="text-[#d8d7cf]"> test snapshots match</span>
                    </code>
                  </pre>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section className="mx-auto my-4 max-w-[1500px] px-3 sm:px-4 md:px-8 lg:px-10">
          <div className="border border-white/10 bg-[#070707] px-4 py-10 sm:px-8 lg:px-12">
            <div className="flex flex-col items-center gap-6 text-center sm:gap-8">
              <div className="flex flex-col items-center">
                <h2 className="text-3xl font-semibold text-[#f8f8f4] sm:text-4xl">
                  Ready to build?
                </h2>
                <p className="mt-3 text-[#c7c6bf]">Install Acton and start from the docs.</p>
              </div>
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
        </section>
      </main>

      <footer className="mx-auto max-w-[1500px] px-3 sm:px-4 md:px-8 lg:px-10">
        <div className="border-x border-t border-white/10 bg-[#070707] p-5 sm:p-7 lg:p-8">
          <div className="grid gap-6 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
            <div>
              <Link href="/" className="inline-flex items-center gap-3">
                <span className="text-xl font-semibold text-white">Acton</span>
              </Link>
              <p className="mt-3 max-w-md text-sm leading-5 text-[#c7c6bf]">
                TON toolchain for building, testing, debugging, and verifying smart contracts.
              </p>
            </div>
            <div className="grid w-full grid-cols-[auto_1fr] items-center gap-4 text-sm text-[#c7c6bf] md:flex md:w-auto md:justify-end">
              <div className="flex items-center gap-1">
                {HEADER_ICON_LINKS.map(({ href, label, icon: Icon }) => (
                  <IconLink key={label} href={href} label={label} icon={Icon} />
                ))}
              </div>
              <div className="justify-self-end text-right">
                Built by{' '}
                <Link
                  href="https://t.me/toncore"
                  target="_blank"
                  className="font-medium text-[#9AE7FF] transition-colors hover:text-white"
                >
                  TON Core
                </Link>
              </div>
            </div>
          </div>
        </div>
      </footer>
    </div>
  );
}

function SiteHeader() {
  return (
    <header className="sticky top-0 z-40 border-b border-white/10 bg-[#050505]/90 backdrop-blur-xl">
      <nav className="mx-auto flex h-14 max-w-[1500px] items-center justify-between px-4 md:px-8 lg:px-10">
        <div className="flex items-center gap-5">
          <Link href="/" className="flex items-center gap-3">
            <Image
              src={logoDark}
              alt=""
              width={28}
              height={28}
              className="h-7 w-7 rounded-md"
              priority
            />
            <span className="text-base font-semibold text-white">Acton</span>
          </Link>
          <span className="hidden h-5 w-px bg-white/14 sm:block" />
          <div className="hidden items-center gap-6 sm:flex">
            <Link href="/docs/welcome" className="text-sm text-[#c9c8c0] transition-colors hover:text-white">Docs</Link>
            <Link href="/docs/commands/overview" className="text-sm text-[#c9c8c0] transition-colors hover:text-white">Commands</Link>
            <Link href="/docs/testing/overview" className="text-sm text-[#c9c8c0] transition-colors hover:text-white">Testing</Link>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1">
            {HEADER_ICON_LINKS.map(({ href, label, icon: Icon }) => (
              <IconLink key={label} href={href} label={label} icon={Icon} />
            ))}
          </div>
        </div>
      </nav>
    </header>
  );
}

function IconLink({
  href,
  label,
  icon: Icon,
}: {
  href: string;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
}) {
  return (
    <Link
      href={href}
      target={href.startsWith('http') ? '_blank' : undefined}
      aria-label={label}
      className="inline-flex h-9 w-9 items-center justify-center rounded-lg text-[#c9c8c0] transition-colors hover:bg-white/[0.06] hover:text-white"
    >
      <Icon className="h-4 w-4" />
    </Link>
  );
}

function TelegramIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        d="M20.86 4.68 17.7 19.56c-.24 1.05-.86 1.3-1.74.8l-4.82-3.55-2.32 2.24c-.26.26-.47.47-.97.47l.34-4.91 8.94-8.08c.39-.34-.08-.53-.6-.19L5.49 13.29.73 11.8c-1.03-.32-1.05-1.03.22-1.53L19.56 3.1c.86-.32 1.61.19 1.3 1.58Z"
        fill="currentColor"
      />
    </svg>
  );
}

function FeatureTitle({ title }: { title: string }) {
  const accent = FEATURE_TITLE_ACCENTS[title];

  if (!accent) {
    return title;
  }

  const accentIndex = title.indexOf(accent);

  if (accentIndex === -1) {
    return title;
  }

  return (
    <>
      {title.slice(0, accentIndex)}
      <span className="text-[#9AE7FF]">{accent}</span>
      {title.slice(accentIndex + accent.length)}
    </>
  );
}

function FeatureIcon({ title }: { title: string }) {
  const config = FEATURE_ICONS[title];

  if (!config) {
    return null;
  }

  const Icon = config.icon;

  return (
    <span className="inline-flex text-[#9AE7FF]">
      <Icon className="h-9 w-9" strokeWidth={1.8} />
    </span>
  );
}

function SectionHeader({
  eyebrow,
  title,
  description,
}: {
  eyebrow: string;
  title: string;
  description: string;
}) {
  return (
    <div className="max-w-3xl">
      <p className="text-xs font-semibold uppercase tracking-[0.18em] text-[#9AE7FF]/70">
        {eyebrow}
      </p>
      <h2 className="mt-4 text-3xl font-semibold leading-tight text-[#f8f8f4] sm:text-5xl">
        {title}
      </h2>
      <p className="mt-5 text-lg leading-7 text-[#c7c6bf] sm:mt-6 sm:text-xl sm:leading-8">
        {description}
      </p>
    </div>
  );
}
