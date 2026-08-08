"use client"

import React, {useState} from "react"
import {Copy, Check} from "lucide-react"

export const INSTALL_COMMAND =
  "curl -LsSf https://github.com/ton-blockchain/acton/releases/latest/download/acton-installer.sh | sh"

export function HighlightedInstallCommand() {
  return (
    <>
      <span className="text-[#9AE7FF]">curl</span>
      <span className="text-sky-200"> -LsSf </span>
      <span className="text-[#ef8cff]">
        https://github.com/ton-blockchain/acton/releases/latest/download/acton-installer.sh
      </span>
      <span className="text-white/35"> | </span>
      <span className="text-[#9AE7FF]">sh</span>
    </>
  )
}

export function InlineInstallationCommand() {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(INSTALL_COMMAND)
    setCopied(true)
    setTimeout(() => setCopied(false), 1600)
  }

  return (
    <div className="inline-flex h-11 w-full max-w-full min-w-0 items-center justify-between gap-3 rounded-full border border-white/10 bg-white/[0.03] px-4 font-mono text-sm">
      <code className="min-w-0 overflow-x-auto whitespace-nowrap [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        <HighlightedInstallCommand />
      </code>
      <button
        type="button"
        onClick={handleCopy}
        aria-label={copied ? "Copied install command" : "Copy install command"}
        className="shrink-0 rounded-md p-1 text-[#8d8c84] transition-colors hover:bg-white/[0.06] hover:text-white"
      >
        {copied ? (
          <Check className="h-3.5 w-3.5 text-[#9AE7FF]" />
        ) : (
          <Copy className="h-3.5 w-3.5" />
        )}
      </button>
    </div>
  )
}
