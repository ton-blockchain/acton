'use client';

import React, {useState} from 'react';
import {Copy, Check} from 'lucide-react';

const TABS = ['macOS / Linux'];
export const INSTALL_COMMAND = 'curl -LsSf https://ton.org/acton/install.sh | sh';

const COMMANDS: Record<string, string> = {
  'macOS / Linux': INSTALL_COMMAND,
}

const highlightCommand = (command: string) => {
  if (command.includes('curl')) {
    return command
      .replace('curl', '<span class="text-purple-400">curl</span>')
      .replace('| sh', '<span class="text-white/50">|</span> <span class="text-purple-400">sh</span>')
  }
  return command;
};

export function HighlightedInstallCommand() {
  return (
    <>
      <span className="text-teal-200">curl</span>
      <span className="text-sky-200"> -LsSf </span>
      <span className="text-[#ef8cff]">https://ton.org/acton/install.sh</span>
      <span className="text-white/35"> | </span>
      <span className="text-teal-200">sh</span>
    </>
  );
}

export function InlineInstallationCommand() {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(INSTALL_COMMAND);
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
  };

  return (
    <div className="inline-flex h-11 w-full max-w-full min-w-0 items-center justify-between gap-3 rounded-full border border-white/10 bg-white/[0.03] px-4 font-mono text-sm">
      <code className="min-w-0 overflow-x-auto whitespace-nowrap [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        <HighlightedInstallCommand />
      </code>
      <button
        type="button"
        onClick={handleCopy}
        aria-label={copied ? 'Copied install command' : 'Copy install command'}
        className="shrink-0 rounded-md p-1 text-[#8d8c84] transition-colors hover:bg-white/[0.06] hover:text-white"
      >
        {copied ? <Check className="h-3.5 w-3.5 text-teal-200" /> : <Copy className="h-3.5 w-3.5" />}
      </button>
    </div>
  );
}

export const InstallationCodeBlock: React.FC = () => {
  const [activeTab, setActiveTab] = useState(TABS[0]);
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    void navigator.clipboard.writeText(COMMANDS[activeTab]);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative max-w-xl mx-auto mt-16">
      <div
        className="absolute -inset-0.5 bg-gradient-to-r from-purple-600 to-blue-500 rounded-2xl blur opacity-20 group-hover:opacity-100 transition duration-1000 group-hover:duration-200"></div>
      <div className="relative bg-black/70 backdrop-blur-md border border-white/10 rounded-2xl shadow-lg">
        <div className="flex border-b border-white/10 px-2 pt-2">
          {TABS.map(tab => (
            <button
              key={tab}
              onClick={() => {
                setActiveTab(tab);
                setCopied(false);
              }}
              className={`px-4 py-2 text-sm font-medium transition-colors focus:outline-none rounded-t-lg ${
                activeTab === tab
                  ? 'text-white bg-white/5'
                  : 'text-white/50 hover:text-white/80'
              }`}
            >
              {tab}
            </button>
          ))}
        </div>
        <div className="p-4 flex items-center justify-between">
          <div className="flex items-center gap-3 overflow-x-auto">
            <span className="text-white/40 select-none text-base font-mono">$</span>
            <code
              className="text-white font-mono text-sm"
              dangerouslySetInnerHTML={{__html: highlightCommand(COMMANDS[activeTab])}}
            />
          </div>
          <button onClick={handleCopy}
                  className="text-white/50 hover:text-white transition-colors p-1.5 rounded-md shrink-0 hover:cursor-pointer">
            {copied ? <Check className="w-4 h-4 text-green-400"/> : <Copy className="w-4 h-4"/>}
          </button>
        </div>
      </div>
    </div>
  );
};
