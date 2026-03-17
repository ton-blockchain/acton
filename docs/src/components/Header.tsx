"use client";
import Link from 'next/link';
import { Github, BookOpen } from 'lucide-react';
import { LiquidGlass } from '@/components/LiquidGlass';

const HEADER_WIDTH = 'min(calc(100vw - 2rem), 80rem)';
const HEADER_HEIGHT = '4.75rem';

function HeaderContent() {
  const linkClassName =
    "inline-flex items-center gap-3 rounded-full px-3 py-2 text-sm font-medium text-white/70 transition-colors hover:text-white md:px-4";

  return (
    <nav className="relative flex h-full w-full items-center justify-between px-5 text-white md:px-8">
      <div className="flex w-full items-center justify-between">
        <div className="flex items-center">
          <span className="bg-gradient-to-b from-white via-white to-white/40 bg-clip-text text-2xl font-bold tracking-tight text-transparent">
            Acton
          </span>
        </div>
        <div className="flex items-center gap-3 md:gap-4">
          <Link
            href="/docs/welcome/"
            className={linkClassName}
          >
            <BookOpen className="h-4 w-4" />
            <span>Documentation</span>
          </Link>
          <Link
            href="https://github.com"
            target="_blank"
            className={linkClassName}
          >
            <Github className="h-4 w-4" />
            <span>GitHub</span>
          </Link>
        </div>
      </div>
    </nav>
  );
}

export function Header() {
  return (
    <header className="fixed left-1/2 top-6 z-[100] w-full -translate-x-1/2 px-4 pointer-events-none">
      <div className="pointer-events-auto mx-auto" style={{ width: HEADER_WIDTH, height: HEADER_HEIGHT }}>
        <LiquidGlass
          className="h-full w-full"
          radius={28}
          depth={14}
          strength={95}
          blur={0}
          chromaticAberration={2}
          tint="charcoal"
          brightness={1.02}
          saturate={1.15}
        >
          <HeaderContent />
        </LiquidGlass>
        <noscript>
          <div style={{ width: HEADER_WIDTH, height: HEADER_HEIGHT }}>
            <HeaderContent />
          </div>
        </noscript>
      </div>
    </header>
  );
}
