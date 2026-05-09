"use client"

import {useState} from "react"
import {Box, Check, Copy} from "lucide-react"

const FUNC2TOLK_PROMPT = `Use $func2tolk to migrate this FunC and TypeScript TON project to Tolk + Acton.

Inspect the existing contracts, wrappers, tests, scripts, and fixtures. Preserve TL-B layouts, opcodes, error codes, getter shapes, send modes, bounce behavior, and observable transaction behavior.

Use acton func2tolk if helpful, refactor the draft into idiomatic Tolk, migrate the important tests, generate wrappers, run acton build and acton test, and finish with a short compatibility self-audit.`

export function Func2TolkPrompt() {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(FUNC2TOLK_PROMPT)
    setCopied(true)
    setTimeout(() => setCopied(false), 1600)
  }

  return (
    <div className="my-6 overflow-hidden rounded-lg border border-fd-border bg-fd-card text-fd-foreground">
      <div className="flex items-center justify-between gap-3 border-b border-fd-border px-4 py-2">
        <span className="text-xs font-medium text-fd-muted-foreground">Prompt</span>
        <button
          type="button"
          onClick={handleCopy}
          className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium text-fd-muted-foreground transition-colors hover:bg-fd-accent hover:text-fd-accent-foreground"
          aria-label={copied ? "Copied prompt" : "Copy prompt"}
        >
          {copied ? <Check className="size-3.5 text-fd-primary" /> : <Copy className="size-3.5" />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <div className="space-y-3 px-4 py-4 text-sm leading-7">
        <p>
          Use{" "}
          <span className="font-semibold text-[#2D83EC] dark:text-[#58A6FF]">
            <Box className="mr-1 inline size-4 align-[-0.15em]" aria-hidden="true" />
            $func2tolk
          </span>{" "}
          to migrate this FunC and TypeScript TON project to Tolk + Acton.
        </p>
        <p>
          Inspect the existing contracts, wrappers, tests, scripts, and fixtures. Preserve TL-B
          layouts, opcodes, error codes, getter shapes, send modes, bounce behavior, and observable
          transaction behavior.
        </p>
        <p>
          Use <code>acton func2tolk</code> if helpful, refactor the draft into idiomatic Tolk,
          migrate the important tests, generate wrappers, run <code>acton build</code> and{" "}
          <code>acton test</code>, and finish with a short compatibility self-audit.
        </p>
      </div>
    </div>
  )
}
