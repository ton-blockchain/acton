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

export const metadata: Metadata = {
  title: 'Acton — TON Development Toolkit',
  description: 'Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.',
  openGraph: {
    title: 'Acton — TON Development Toolkit',
    description: 'Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.',
    url: 'i582.github.io/acton',
    locale: 'en_US',
    type: 'website',
  },
  twitter: {
    card: 'summary_large_image',
    title: 'Acton — TON Development Toolkit',
    description: 'Acton is a blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.',
  },
};

interface Feature {
  title: string;
  description: string;
}

const FEATURES: Feature[] = [
  {
    title: "Native Tolk Testing",
    description: "Write integration and unit tests directly in Tolk without TypeScript wrappers. Test individual functions or whole contract systems with unified API."
  },
  {
    title: "Smart Contract Compilation",
    description: "Compile Tolk source files to TVM bytecode with incremental caching. Generate source maps for debugging and export compiled code in multiple formats."
  },
  {
    title: "Tolk Formatter",
    description: "Automatically format Tolk code with consistent indentation, spacing, and style. Maintain clean, readable code across your entire project."
  },
  {
    title: "Dependency Management",
    description: "Configure contract dependencies in Acton.toml. Choose between code embedding, library references, or storage deployment. Automatic dependency resolution and circular dependency detection."
  },
  {
    title: "Standalone Tolk Scripts",
    description: "Execute Tolk files as standalone scripts. Perfect for experimentation, deployment automation, and blockchain interaction using real wallets or test environments."
  },
  {
    title: "Contract Verification",
    description: "Verify deployed contracts match their source code using the TON Verifier service. Supports both testnet and mainnet."
  },
  {
    title: "Blockchain Integration",
    description: "Deploy contracts and interact with live blockchain state. Fork mainnet/testnet state for testing, broadcast transactions with real wallets, and query contract methods across networks."
  }
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
                <h1 className="text-6xl md:text-9xl font-bold tracking-tighte">
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

              <div className="flex flex-wrap gap-6 justify-center">
                <Link href="/docs/installation">
                  <Button size="lg"
                          className="glass-button h-12 px-20 rounded-2xl text-base bg-white/10 text-white border border-white/10">
                    Get Started
                  </Button>
                </Link>
                <Link href="https://github.com" target="_blank">
                  <Button size="lg" variant="outline"
                          className="glass-button-outline h-12 px-20 rounded-2xl text-base border-white/10 hover:bg-white/5">
                    <Github className="w-4 h-4 mr-2"/>
                    GitHub
                  </Button>
                </Link>
              </div>
            </div>

            {/* Features Section */}
            <div className="features-container">
              {FEATURES.map((feature, index) => (
                <div
                  key={feature.title}
                  className={index % 2 === 0 ? "feature-row" : "feature-row-reverse"}
                >
                  <div className={index % 2 === 0 ? "feature-text" : "feature-text-reverse"}>
                    <h2 className="text-5xl md:text-6xl font-bold tracking-tighter text-white">
                      {feature.title}
                    </h2>
                    <p className="text-xl md:text-2xl text-white/50 leading-relaxed font-light">
                      {feature.description}
                    </p>
                  </div>

                  <div className="feature-image">
                    {/* placeholder */}
                  </div>
                </div>
              ))}
            </div>

            <div className="relative z-10 mt-64 text-center space-y-12">
              <h2 className="text-5xl md:text-7xl font-bold tracking-tighter">
                Ready to build?
              </h2>
            </div>

            <InstallationCodeBlock/>

            <div className="flex flex-wrap gap-6 justify-center pt-12">
              <Link href="/docs/installation">
                <Button size="lg"
                        className="glass-button h-12 px-20 rounded-2xl text-base bg-white/10 text-white border border-white/10">
                  Get Started
                </Button>
              </Link>
              <Link href="https://github.com" target="_blank">
                <Button size="lg" variant="outline"
                        className="glass-button-outline h-12 px-20 rounded-2xl text-base border-white/10 hover:bg-white/5">
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
                <p className="text-sm text-white/40">TON development toolkit</p>
              </div>
              <div className="flex gap-8 text-sm text-white/40">
                <Link href="/privacy" className="hover:text-white transition-colors">Privacy</Link>
                <Link href="/terms" className="hover:text-white transition-colors">Terms</Link>
                <Link href="https://github.com" className="hover:text-white transition-colors">GitHub</Link>
              </div>
            </div>
          </div>
        </footer>
      </div>
    </div>
  );
}
