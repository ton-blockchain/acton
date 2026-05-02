"use client";

import { Search } from 'lucide-react';
import { useSearchContext } from 'fumadocs-ui/contexts/search';

export function HeaderSearchField() {
  const { hotKey, setOpenSearch } = useSearchContext();

  return (
    <>
      <button
        type="button"
        aria-label="Search docs"
        className="mr-4 h-9 w-[220px] items-center gap-2 rounded-lg border border-white/10 bg-white/[0.04] px-3 text-sm text-[#c9c8c0] transition-colors hover:bg-white/[0.07] hover:text-white max-sm:hidden sm:flex lg:w-[280px]"
        onClick={() => setOpenSearch(true)}
      >
        <Search className="h-4 w-4 shrink-0" />
        <span>Search</span>
        <span className="ml-auto inline-flex gap-0.5">
          {hotKey.map((key, index) => (
            <kbd
              key={index}
              className="rounded-md border border-white/10 bg-white/[0.05] px-1.5 text-xs text-[#aaa9a1]"
            >
              {key.display}
            </kbd>
          ))}
        </span>
      </button>
      <button
        type="button"
        aria-label="Search docs"
        className="flex h-9 w-9 items-center justify-center rounded-lg text-[#c9c8c0] transition-colors hover:bg-white/[0.06] hover:text-white sm:hidden"
        onClick={() => setOpenSearch(true)}
      >
        <Search className="h-4 w-4" />
      </button>
    </>
  );
}
