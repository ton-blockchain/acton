'use client';

import React, {useState, useEffect} from 'react';
import {Copy, Check} from 'lucide-react';

const TABS = ['macOS / Linux', 'Windows'];
const COMMANDS: Record<string, string> = {
  "macOS / Linux":
    "curl -fsSL https://github.com/i582/acton-public/releases/latest/download/acton-installer.sh | sh",
  Windows: 'powershell -c "irm acton.sh/install.ps1 | iex"',
}

const highlightCommand = (command: string) => {
  if (command.includes('curl')) {
    return command
      .replace('curl', '<span class="text-purple-400">curl</span>')
      .replace('|', '<span class="text-white/50">|</span>')
      .replace('bash', '<span class="text-purple-400">bash</span>');
  }
  if (command.includes('powershell')) {
    return command
      .replace('powershell', '<span class="text-purple-400">powershell</span>')
      .replace('-c', '<span class="text-blue-400">-c</span>')
      .replace('"irm acton.sh/install.ps1 | iex"', '<span class="text-green-400">"irm acton.sh/install.ps1 | iex"</span>')
      .replace('|', '<span class="text-white/50">|</span>');
  }
  return command;
};

export const InstallationCodeBlock: React.FC = () => {
  const [activeTab, setActiveTab] = useState(TABS[0]);
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    void navigator.clipboard.writeText(COMMANDS[activeTab]);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  useEffect(() => {
    setCopied(false);
  }, [activeTab]);

  return (
    <div className="relative max-w-4xl mx-auto mt-16">
      <div
        className="absolute -inset-0.5 bg-gradient-to-r from-purple-600 to-blue-500 rounded-2xl blur opacity-20 group-hover:opacity-100 transition duration-1000 group-hover:duration-200"></div>
      <div className="relative bg-black/70 backdrop-blur-md border border-white/10 rounded-2xl shadow-lg">
        <div className="flex border-b border-white/10 px-2 pt-2">
          {TABS.map(tab => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
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
