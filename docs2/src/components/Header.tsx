"use client";
import Link from 'next/link';
import { Github, BookOpen } from 'lucide-react';

export function Header() {
  return (
    <header className="fixed top-6 left-1/2 transform -translate-x-1/2 z-[100] max-w-7xl w-full transition-all duration-300 group px-4 md:px-0">
      <div className="absolute inset-0 rounded-2xl overflow-hidden transition-all duration-300">
        {/* Base Glass Layer */}
        <div 
          className="absolute inset-0 bg-[rgba(20,20,20,0.15)]"
          style={{
            backdropFilter: "blur(40px) saturate(200%)",
            WebkitBackdropFilter: "blur(40px) saturate(200%)",
          }}
        />
        
        {/* Noise Texture */}
        <div 
          className="absolute inset-0 opacity-[0.04] mix-blend-overlay pointer-events-none"
          style={{
            backgroundImage: `url("data:image/svg+xml,%3Csvg viewBox='0 0 200 200' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.8' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E")`,
          }}
        />

        {/* Inner Shine/Refraction Simulation */}
        <div className="absolute inset-0 rounded-2xl shadow-[inset_0_1px_0_0_rgba(255,255,255,0.2),inset_0_0_40px_0_rgba(255,255,255,0.1)]" />
        
        {/* Top Specular Highlight - Thicker and brighter */}
        <div className="absolute top-0 left-[10%] right-[10%] h-[2px] bg-gradient-to-r from-transparent via-white/50 to-transparent opacity-70 blur-[0.5px]" />
        
        {/* Bottom Rim Light */}
        <div className="absolute bottom-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-white/20 to-transparent" />
      </div>

      {/* Content */}
      <nav className="relative px-6 py-4 flex items-center justify-between">
        <div className="text-2xl font-bold tracking-tight flex items-center gap-3">
          <span className="bg-gradient-to-b from-white via-white to-white/40 bg-clip-text text-transparent">
            Acton
          </span>
        </div>
        <div className="flex gap-8 items-center">
          <Link href="/docs/installation/"
                className="text-sm font-medium text-white/70 hover:text-white transition-colors flex items-center gap-3">
            <BookOpen className="w-4 h-4"/>
            Documentation
          </Link>
          <Link href="https://github.com" target="_blank"
                className="text-sm font-medium text-white/70 hover:text-white transition-colors flex items-center gap-2">
            <Github className="w-4 h-4"/>
            GitHub
          </Link>
        </div>
      </nav>
    </header>
  );
}
