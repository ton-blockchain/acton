import {Metadata} from 'next';
import Link from 'next/link';
import {Button} from '@/components/ui/button';
import {
  Github
} from 'lucide-react';
import DotGrid from "@/components/Grid";
import {Typewriter} from '@/components/Typewriter';
import {InstallationCodeBlock} from '@/components/InstallationCodeBlock';
import {Header} from '@/components/Header';
import {LandingVideo} from '@/components/LandingVideo';

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
    description2: 'TON Connect and AppKit as the UI — Tolk and TON Blockchain as the backend.',
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
  }
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

export default function Home() {
  return (
    <div className="relative min-h-screen flex flex-col bg-black text-white overflow-x-hidden z-0">
      <div style={{
        position: 'absolute',
        top: 0,
        left: -10,
        width: '100%',
        height: '900px',
        zIndex: 0,
        maskImage: 'radial-gradient(ellipse at center, black 25%, transparent 50%), linear-gradient(to bottom, transparent 0%, black 35%, black 80%, transparent 100%)',
        WebkitMaskImage: 'radial-gradient(ellipse at center, black 25%, transparent 50%), linear-gradient(to bottom, transparent 0%, black 20%, black 80%, transparent 100%)'
      }}>
        <DotGrid
          dotSize={3}
          gap={30}
          baseColor="#404040"
          activeColor="#5227FF"
          proximity={120}
          shockRadius={250}
          shockStrength={5}
          resistance={750}
          returnDuration={1.5}
        />
      </div>

      {/* Bottom section background */}
      <div style={{
        position: 'absolute',
        bottom: 350,
        left: -10,
        width: '100%',
        height: '800px',
        zIndex: 0,
        maskImage: 'radial-gradient(ellipse at center, black 25%, transparent 50%), linear-gradient(to top, transparent 0%, black 30%, black 70%, transparent 100%)',
        WebkitMaskImage: 'radial-gradient(ellipse at center, black 25%, transparent 50%), linear-gradient(to top, transparent 0%, black 30%, black 70%, transparent 100%)'
      }}>
        <DotGrid
          dotSize={3}
          gap={30}
          baseColor="#404040"
          activeColor="#5227FF"
          proximity={120}
          shockRadius={250}
          shockStrength={5}
          resistance={750}
          returnDuration={1.5}
        />
      </div>
      <div className="relative z-50 flex-1 flex flex-col">
        <Header/>

        <main className="flex-1 flex flex-col items-center z-50">
          <div className="container mx-auto max-w-7xl px-6 py-32 md:py-65">
            <div className="text-center space-y-16 mb-48">
              <div className="space-y-0.5">
                <h1 className="text-7xl md:text-9xl font-bold tracking-tight">
                  <span className="bg-gradient-to-b from-white via-white to-white/40 bg-clip-text text-transparent">
                    Acton
                  </span>
                </h1>
                <p className="text-3xl md:text-2xl text-white/60 max-w-2.5xl mx-auto font-light leading-relaxed mt-4">
                  Blazingly fast <Typewriter words={['toolkit', 'test runner', 'build system', 'formatter', 'verifier']}
                                             className="font-normal" style={{color: "#5227FF"}}/> for TON smart contract
                  development
                </p>
              </div>

              <InstallationCodeBlock/>
            </div>

            <section className="relative z-10 mb-40 space-y-10">
              <div className="max-w-5xl space-y-5">
                <span className="inline-flex w-fit items-center gap-3 rounded-full border border-white/10 bg-white/[0.03] px-4 py-2 text-xs font-semibold uppercase tracking-[0.24em] text-white/55">
                  <span className="h-2 w-2 rounded-full bg-[#7b61ff] shadow-[0_0_18px_rgba(123,97,255,0.85)]"/>
                  From zero to testnet
                </span>
                <div className="space-y-5">
                  <h2 className="max-w-5xl text-5xl font-thin tracking-relaxed text-white md:text-7xl">
                    2-minute walkthrough
                  </h2>
                  <p className="max-w-3xl text-lg font-light leading-relaxed text-white/58 md:text-2xl">
                    Tiny Acton workshop: create a project, launch tests, guide through essential features,
                    airdrop&nbsp;TONs to your wallet, and deploy a contract to TON Blockchain.
                  </p>
                </div>
              </div>

              <div className="landing-video-shell">
                <div className="aspect-video flex items-center justify-center bg-[radial-gradient(circle_at_top_left,rgba(123,97,255,0.18),transparent_30%),radial-gradient(circle_at_bottom_right,rgba(68,178,255,0.12),transparent_34%),linear-gradient(180deg,rgba(8,8,12,0.98)_0%,rgba(5,5,8,0.94)_100%)] px-8 text-center">
                  <div className="max-w-3xl space-y-5">
                    <p className="text-xs font-semibold uppercase tracking-[0.24em] text-white/42">
                      Placeholder
                    </p>
                    <h3 className="text-2xl font-light tracking-relaxed text-white md:text-6xl">
                      I will record this video the next week
                    </h3>
                  </div>
                </div>
                {/*<video*/}
                {/*  className="landing-video"*/}
                {/*  controls*/}
                {/*  playsInline*/}
                {/*  preload="metadata"*/}
                {/*  src={SHOWCASE_VIDEO_URL}*/}
                {/*>*/}
                {/*  Your browser does not support the video tag.*/}
                {/*</video>*/}
              </div>
            </section>

            <section className="relative z-10 mb-40 space-y-30">
              <div className="max-w-5xl space-y-5">
                <span className="inline-flex w-fit items-center gap-3 rounded-full border border-white/10 bg-white/[0.03] px-4 py-2 text-xs font-semibold uppercase tracking-[0.24em] text-white/55">
                  <span className="h-2 w-2 rounded-full bg-[#7b61ff] shadow-[0_0_18px_rgba(123,97,255,0.85)]"/>
                  Essential features
                </span>
                <div className="space-y-5">
                  <h2 className="max-w-5xl text-5xl font-thin tracking-relaxed text-white md:text-7xl">
                    A unified toolchain for TON
                  </h2>
                  <p className="max-w-3xl text-lg font-light leading-relaxed text-white/58 md:text-2xl">
                    Not another small utility. Not another isolated CLI.<br />
                    A full development environment built as one coherent system — around Tolk.
                  </p>
                </div>
              </div>

              <div className="space-y-30">
                {VIDEO_FEATURES.map((feature, index) => {
                  const reversed = index % 2 === 1;

                  return (
                    <article
                      key={feature.title}
                      className={`grid gap-8 lg:items-start lg:gap-16 ${
                        reversed
                          ? "lg:grid-cols-[minmax(0,1.2fr)_minmax(0,0.8fr)]"
                          : "lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]"
                      }`}
                    >
                      <div
                        className={`space-y-6 ${reversed ? "lg:order-2 lg:text-right" : "lg:order-1"}`}
                      >
                        <div
                          className={`space-y-4 ${reversed ? "max-w-xl lg:ml-auto" : "max-w-xl"}`}
                        >
                          <h3 className="text-4xl font-light tracking-relaxed text-white md:text-4xl">
                            {feature.title}
                          </h3>
                          <p className="text-lg font-light leading-relaxed text-white/52 md:text-2xl">
                            {feature.description}
                          </p>
                          <p className="text-lg font-light leading-relaxed text-white/52 md:text-2xl">
                            {feature.description2}
                          </p>
                        </div>
                      </div>

                      <div className={reversed ? "lg:order-1" : "lg:order-2"}>
                        <div className="landing-video-shell landing-video-shell-compact">
                          <LandingVideo
                            className="landing-video-compact"
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
            </section>

            <section className="relative z-10 mb-40 space-y-10">
              <div className="max-w-4xl space-y-5">
                <span className="inline-flex w-fit items-center gap-3 rounded-full border border-white/10 bg-white/[0.03] px-4 py-2 text-xs font-semibold uppercase tracking-[0.24em] text-white/55">
                  <span className="h-2 w-2 rounded-full bg-[#7b61ff] shadow-[0_0_18px_rgba(123,97,255,0.85)]"/>
                  Extra depth
                </span>
                <div className="space-y-5">
                  <h2 className="max-w-5xl text-5xl font-thin tracking-relaxed text-white md:text-7xl">
                    More than the happy path
                  </h2>
                  <p className="max-w-4xl text-lg font-light leading-relaxed text-white/58 md:text-2xl">
                    Several principle features that allow dive deep into contract development.<br />
                    Focused on security, error prevention, and gas optimization.
                  </p>
                </div>
              </div>

              <div className="grid gap-5 md:grid-cols-2 xl:grid-cols-3">
                {BELOW_FEATURES.map(feature => (
                  <article
                    key={feature.title}
                    className="supporting-feature-card"
                  >
                    <div className="space-y-4">
                      <h3 className="text-2xl font-light tracking-relaxed text-white md:text-4xl">
                        {feature.title}
                      </h3>
                      <p className="max-w-2xl text-lg font-light leading-relaxed text-white/58">
                        {feature.description}
                      </p>
                    </div>
                    <Link href={feature.docLink} className="supporting-feature-link">
                      Read in docs <span aria-hidden="true">→</span>
                    </Link>
                  </article>
                ))}
              </div>
            </section>

            <section className="relative z-10 mb-12 space-y-10">
              <div className="max-w-4xl space-y-5">
                <span className="inline-flex w-fit items-center gap-3 rounded-full border border-white/10 bg-white/[0.03] px-4 py-2 text-xs font-semibold uppercase tracking-[0.24em] text-white/55">
                  <span className="h-2 w-2 rounded-full bg-[#7b61ff] shadow-[0_0_18px_rgba(123,97,255,0.85)]"/>
                  FunC migration
                </span>
                <div className="space-y-5">
                  <h2 className="max-w-5xl text-5xl font-thin tracking-relaxed text-white md:text-7xl">
                    FunC to Tolk in one click
                  </h2>
                  <p className="max-w-3xl text-lg font-light leading-relaxed text-white/58 md:text-2xl">
                    Convert your existing FunC project to Tolk with a single command,<br />
                    and then iteratively refactor it keeping green tests.
                  </p>
                </div>
              </div>

              <div className="landing-video-shell">
                <div className="aspect-video flex items-center justify-center bg-[radial-gradient(circle_at_top_left,rgba(123,97,255,0.18),transparent_30%),radial-gradient(circle_at_bottom_right,rgba(68,178,255,0.12),transparent_34%),linear-gradient(180deg,rgba(8,8,12,0.98)_0%,rgba(5,5,8,0.94)_100%)] px-8 text-center">
                  <div className="max-w-3xl space-y-5">
                    <p className="text-xs font-semibold uppercase tracking-[0.24em] text-white/42">
                      Placeholder
                    </p>
                    <h3 className="text-2xl font-light tracking-relaxed text-white md:text-6xl">
                      todo think about this block, try to skill LLMs
                    </h3>
                  </div>
                </div>
              </div>
            </section>

            <div className="relative z-10 mt-64 text-center space-y-12">
              <h2 className="text-5xl md:text-7xl font-bold tracking-tighter">
                Ready to build?
              </h2>
            </div>

            <InstallationCodeBlock/>

              <div className="flex flex-wrap gap-6 justify-center pt-12">
                <Link href="/docs/welcome">
                  <Button size="lg"
                          className="glass-button cursor-pointer h-12 px-20 rounded-2xl text-base bg-white/10 text-white border border-white/10">
                    Get Started
                  </Button>
                </Link>
                <Link href="https://github.com/ton-blockchain/acton" target="_blank">
                  <Button size="lg" variant="outline"
                          className="glass-button-outline cursor-pointer h-12 px-20 rounded-2xl text-base border-white/10 hover:bg-white/5">
                    <Github className="w-4 h-4 mr-2"/>
                    GitHub
                  </Button>
                </Link>
              </div>
          </div>
        </main>

        <footer className="border-t border-white/10 py-12 mt-32 bg-black">
          <div className="container mx-auto px-6">
            <div className="flex flex-col md:flex-row justify-start items-start gap-8">
              <div className="flex flex-col items-start gap-2">
                <span className="text-2xl font-bold tracking-tight text-white">Acton</span>
                <p className="text-sm text-white/40">TON toolchain</p>
              </div>
            </div>
          </div>
        </footer>
      </div>
    </div>
  );
}
