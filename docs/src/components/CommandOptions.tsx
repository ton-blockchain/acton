import type { ReactNode } from 'react';

export function CommandOptions({ children }: { children: ReactNode }) {
  return <div className="my-6 flex flex-col gap-4">{children}</div>;
}

export function CommandOption({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-xl border border-fd-border/80 bg-fd-card/30 px-5 py-4 shadow-sm">
      {children}
    </div>
  );
}

export function CommandOptionTitle({ children }: { children: ReactNode }) {
  return (
    <div className="mb-3 text-[0.95rem] font-semibold leading-6 tracking-tight text-fd-foreground [&_p]:m-0">
      {children}
    </div>
  );
}
