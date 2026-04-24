import {
  useState,
  useRef,
  useEffect,
  type ChangeEvent,
  type ReactNode,
} from 'react';
import { Check, ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Label } from '@/components/ui/label';

export function Field({
  label,
  hint,
  error,
  children,
  right,
}: {
  label: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  children: ReactNode;
  right?: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <Label className="text-[10.5px] tracking-widest uppercase text-muted-foreground font-semibold">
          {label}
        </Label>
        {right}
      </div>
      {children}
      {error ? (
        <div className="text-[11.5px] text-destructive">{error}</div>
      ) : hint ? (
        <div className="text-[11.5px] text-muted-foreground">{hint}</div>
      ) : null}
    </div>
  );
}

export function Segmented<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: ReactNode }[];
}) {
  return (
    <div className="inline-flex bg-secondary border border-border rounded-lg p-0.5 gap-0.5">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          className={cn(
            'inline-flex items-center gap-1.5 px-2.5 py-1 border-0 rounded-md text-xs font-medium cursor-pointer transition-all',
            value === o.value
              ? 'bg-card text-foreground shadow-sm'
              : 'bg-transparent text-muted-foreground hover:text-foreground',
          )}
          onClick={() => onChange(o.value)}
        >
          <span>{o.label}</span>
        </button>
      ))}
    </div>
  );
}

export function Select<T extends string>({
  value,
  onChange,
  options,
  placeholder,
}: {
  value: T | null | undefined;
  onChange: (v: T) => void;
  options: { value: T; label: ReactNode }[];
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const current = options.find((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    function onDocClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener('mousedown', onDocClick);
    return () => document.removeEventListener('mousedown', onDocClick);
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        className={cn(
          'w-full flex items-center justify-between bg-secondary border border-border rounded-lg px-3 py-2 text-sm cursor-pointer whitespace-nowrap overflow-hidden gap-2 transition-all',
          open && 'border-ring ring-ring/50 ring-[3px]',
        )}
        onClick={() => setOpen((v) => !v)}
      >
        <span
          className={cn(
            'overflow-hidden text-ellipsis whitespace-nowrap min-w-0 flex-1 text-left',
            !current && 'text-muted-foreground',
          )}
        >
          {current ? current.label : (placeholder ?? 'Select…')}
        </span>
        <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
      </button>
      {open ? (
        <div className="absolute top-[calc(100%+4px)] left-0 right-0 z-10 bg-popover border border-border rounded-lg p-1 shadow-md flex flex-col gap-px max-h-80 overflow-y-auto">
          {options.length === 0 ? (
            <div className="px-2.5 py-2 text-sm text-muted-foreground">
              No options
            </div>
          ) : (
            options.map((o) => (
              <button
                key={o.value}
                type="button"
                className={cn(
                  'flex items-center justify-between px-2.5 py-2 bg-transparent border-0 rounded-md text-sm text-left cursor-pointer hover:bg-accent',
                  o.value === value && 'text-primary',
                )}
                onClick={() => {
                  onChange(o.value);
                  setOpen(false);
                }}
              >
                {o.label}
                {o.value === value ? <Check className="size-3.5" /> : null}
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}

export function KV({
  k,
  v,
  mono = true,
}: {
  k: ReactNode;
  v: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex justify-between items-baseline py-2.5 border-b border-dashed border-border last:border-b-0 gap-4">
      <span className="text-[10.5px] tracking-widest uppercase text-muted-foreground font-semibold">
        {k}
      </span>
      <span
        className={cn(
          'text-sm text-foreground text-right overflow-hidden text-ellipsis whitespace-nowrap max-w-[60%]',
          mono && 'font-mono text-xs',
        )}
      >
        {v}
      </span>
    </div>
  );
}

export function hashHue(s = '') {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h) % 360;
}

export function Avatar({
  label = '?',
  image,
  size = 48,
  tone,
}: {
  label?: string;
  image?: string | null;
  size?: number;
  tone?: number;
}) {
  const hue = tone ?? hashHue(label);
  const bg = `linear-gradient(135deg, oklch(0.58 0.15 ${hue}), oklch(0.42 0.13 ${hue + 40}))`;
  return (
    <div
      className="rounded-xl overflow-hidden flex items-center justify-center text-white font-semibold shrink-0 shadow-[inset_0_0_0_1px_rgba(255,255,255,0.06)]"
      style={{
        width: size,
        height: size,
        background: image ? 'transparent' : bg,
      }}
    >
      {image ? (
        <img
          src={image}
          alt=""
          className="w-full h-full object-cover"
          onError={(e) => {
            (e.currentTarget as HTMLImageElement).style.display = 'none';
          }}
        />
      ) : (
        <span style={{ fontSize: size * 0.42 }}>
          {String(label).slice(0, 1).toUpperCase()}
        </span>
      )}
    </div>
  );
}

export function placeholderUrl(seed: string, label = '') {
  const hue = hashHue(seed);
  const a = `oklch(0.55 0.12 ${hue})`;
  const b = `oklch(0.38 0.10 ${hue + 30})`;
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='240' height='240' viewBox='0 0 240 240'>
    <defs><pattern id='p' width='10' height='10' patternTransform='rotate(45)' patternUnits='userSpaceOnUse'>
    <rect width='10' height='10' fill='${a}'/><line x1='0' y1='0' x2='0' y2='10' stroke='${b}' stroke-width='4'/>
    </pattern></defs>
    <rect width='240' height='240' fill='url(#p)'/>
    <text x='50%' y='54%' font-family='ui-monospace,monospace' font-size='22' fill='rgba(255,255,255,.85)' text-anchor='middle' font-weight='600'>${label}</text>
  </svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

export function onlyDigits(e: ChangeEvent<HTMLInputElement>) {
  return e.target.value.replace(/[^0-9]/g, '');
}

export function shortAddr(a: string, head = 4, tail = 4) {
  if (!a) return '';
  if (a.length <= head + tail + 1) return a;
  return `${a.slice(0, head)}…${a.slice(-tail)}`;
}
