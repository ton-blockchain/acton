"use client";

import type { CSSProperties, ReactNode } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { cn } from '@/lib/cn';
import { getDisplacementFilter } from '@/utils/liquidGlass';

type LiquidGlassProps = {
  children: ReactNode;
  className?: string;
  contentClassName?: string;
  style?: CSSProperties;
  radius?: number;
  depth?: number;
  strength?: number;
  chromaticAberration?: number;
  blur?: number;
  tint?: 'black' | 'charcoal' | 'gray' | 'white' | 'transparent';
  brightness?: number;
  saturate?: number;
};

function getDiagonalRimGradient(
  lightStrong: string,
  lightSoft: string,
  shadowStrong: string,
  shadowSoft: string,
  midTone: string,
) {
  return [
    `radial-gradient(135% 135% at 0% 0%, ${lightStrong} 0%, ${lightSoft} 24%, transparent 56%)`,
    `radial-gradient(135% 135% at 100% 100%, ${lightStrong} 0%, ${lightSoft} 24%, transparent 56%)`,
    `radial-gradient(135% 135% at 100% 0%, ${shadowStrong} 0%, ${shadowSoft} 26%, transparent 58%)`,
    `radial-gradient(135% 135% at 0% 100%, ${shadowStrong} 0%, ${shadowSoft} 26%, transparent 58%)`,
    `linear-gradient(135deg, ${lightSoft} 0%, ${midTone} 42%, ${shadowSoft} 50%, ${lightSoft} 100%)`,
  ].join(', ');
}

function getTintStyles(tint: LiquidGlassProps['tint']) {
  switch (tint) {
    case 'charcoal':
      return {
        overlay: 'rgba(17, 24, 39, 0.18)',
        glass: 'rgba(17, 24, 39, 0.3)',
        boxShadow: 'inset 0 0 3px 0 rgba(255, 255, 255, 0.12)',
        rimGradient: getDiagonalRimGradient(
          'rgba(255, 255, 255, 0.16)',
          'rgba(255, 255, 255, 0.07)',
          'rgba(15, 23, 42, 0.3)',
          'rgba(15, 23, 42, 0.12)',
          'rgba(255, 255, 255, 0.02)',
        ),
        rimShadow: '0 8px 22px rgba(0, 0, 0, 0.16)',
        rimGlow: 'inset 0 0 0 1px rgba(255, 255, 255, 0.03)',
        filter: 'brightness(0.88) saturate(0.94)',
      };
    case 'gray':
      return {
        overlay: 'rgba(107, 114, 128, 0.16)',
        glass: 'rgba(75, 85, 99, 0.24)',
        boxShadow: 'inset 0 0 4px 0 rgba(255, 255, 255, 0.28)',
        rimGradient: getDiagonalRimGradient(
          'rgba(255, 255, 255, 0.22)',
          'rgba(255, 255, 255, 0.1)',
          'rgba(51, 65, 85, 0.24)',
          'rgba(51, 65, 85, 0.1)',
          'rgba(255, 255, 255, 0.03)',
        ),
        rimShadow: '0 10px 30px rgba(15, 23, 42, 0.16)',
        rimGlow: 'inset 0 0 0 1px rgba(255, 255, 255, 0.05)',
        filter: 'brightness(0.92) saturate(0.96)',
      };
    case 'white':
      return {
        overlay: 'rgba(255, 255, 255, 0.18)',
        glass: 'rgba(250, 250, 250, 0.5)',
        boxShadow: 'inset 0 0 4px 0 rgba(250, 250, 250, 0.5)',
        rimGradient: getDiagonalRimGradient(
          'rgba(255, 255, 255, 0.34)',
          'rgba(255, 255, 255, 0.18)',
          'rgba(148, 163, 184, 0.22)',
          'rgba(148, 163, 184, 0.08)',
          'rgba(255, 255, 255, 0.06)',
        ),
        rimShadow: '0 12px 30px rgba(148, 163, 184, 0.12)',
        rimGlow: 'inset 0 0 0 1px rgba(255, 255, 255, 0.08)',
        filter: undefined as string | undefined,
      };
    case 'transparent':
      return {
        overlay: 'rgba(255, 255, 255, 0.1)',
        glass: 'rgba(9, 9, 11, 0)',
        boxShadow: 'inset 0 0 4px 0 rgba(250, 250, 250, 0.5)',
        rimGradient: getDiagonalRimGradient(
          'rgba(255, 255, 255, 0.24)',
          'rgba(255, 255, 255, 0.1)',
          'rgba(51, 65, 85, 0.22)',
          'rgba(51, 65, 85, 0.08)',
          'rgba(255, 255, 255, 0.03)',
        ),
        rimShadow: '0 10px 24px rgba(15, 23, 42, 0.1)',
        rimGlow: 'inset 0 0 0 1px rgba(255, 255, 255, 0.04)',
        filter: undefined as string | undefined,
      };
    case 'black':
    default:
      return {
        overlay: 'rgba(0, 0, 0, 0.3)',
        glass: 'rgba(9, 9, 11, 0.5)',
        boxShadow: 'inset 0 0 4px 0 rgba(250, 250, 250, 0.5)',
        rimGradient: getDiagonalRimGradient(
          'rgba(255, 255, 255, 0.2)',
          'rgba(255, 255, 255, 0.08)',
          'rgba(15, 23, 42, 0.26)',
          'rgba(15, 23, 42, 0.1)',
          'rgba(255, 255, 255, 0.02)',
        ),
        rimShadow: '0 12px 28px rgba(0, 0, 0, 0.24)',
        rimGlow: 'inset 0 0 0 1px rgba(255, 255, 255, 0.04)',
        filter: 'brightness(0.6)',
      };
  }
}

export function LiquidGlass({
  children,
  className,
  contentClassName,
  style,
  radius = 28,
  depth = 10,
  strength = 100,
  chromaticAberration = 2,
  blur = 0,
  tint = 'black',
  brightness = 1.1,
  saturate = 1.5,
}: LiquidGlassProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const supportsBackdropFilterUrl = useMemo(() => {
    if (typeof document === 'undefined') {
      return false;
    }

    const testEl = document.createElement('div');
    testEl.style.cssText = 'backdrop-filter: url(#test)';
    return (
      testEl.style.backdropFilter === 'url(#test)' ||
      testEl.style.backdropFilter === 'url("#test")'
    );
  }, []);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) {
      return;
    }

    const updateSize = () => {
      const rect = element.getBoundingClientRect();
      setSize({
        width: Math.round(rect.width),
        height: Math.round(rect.height),
      });
    };

    updateSize();

    const observer = new ResizeObserver(updateSize);
    observer.observe(element);
    window.addEventListener('resize', updateSize);

    return () => {
      observer.disconnect();
      window.removeEventListener('resize', updateSize);
    };
  }, []);

  const filterUrl = useMemo(() => {
    if (size.width === 0 || size.height === 0) {
      return null;
    }

    return getDisplacementFilter({
      width: size.width,
      height: size.height,
      radius,
      depth,
      strength,
      chromaticAberration,
    });
  }, [chromaticAberration, depth, radius, size.height, size.width, strength]);

  const tintStyles = getTintStyles(tint);
  const fallbackBackdrop = `blur(${Math.max(size.width / 10, 20)}px) saturate(180%)`;
  const glassBackdrop =
    supportsBackdropFilterUrl && filterUrl
      ? `blur(${blur / 2}px) url('${filterUrl}') blur(${blur}px) brightness(${brightness}) saturate(${saturate})`
      : fallbackBackdrop;

  return (
    <div
      ref={containerRef}
      className={cn('relative overflow-hidden', className)}
      style={{
        ...style,
        borderRadius: `${radius}px`,
      }}
    >
      <div
        className="absolute inset-0 z-[1]"
        style={{ background: tintStyles.overlay }}
      />
      <div className="absolute inset-0 z-[2]">
        <div
          className="h-full w-full"
          style={{
            width: size.width || '100%',
            height: size.height || '100%',
            borderRadius: `${radius}px`,
            background: tintStyles.glass,
            boxShadow: tintStyles.boxShadow,
            filter: tintStyles.filter,
            backdropFilter: glassBackdrop,
            WebkitBackdropFilter: glassBackdrop,
          }}
        />
      </div>
      <div
        className="pointer-events-none absolute inset-0 z-[3]"
        style={{
          borderRadius: `${radius}px`,
          border: '1px solid transparent',
          boxShadow: `${tintStyles.rimShadow}, ${tintStyles.rimGlow}`,
        }}
      />
      <div className={cn('relative z-[4] flex h-full w-full items-center justify-center text-center', contentClassName)}>
        {children}
      </div>
    </div>
  );
}
