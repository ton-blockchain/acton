import React from 'react';

interface CodeTerminalProps {
  code: string;
  filename?: string;
  className?: string;
}

export const CodeTerminal: React.FC<CodeTerminalProps> = ({
                                                            code,
                                                            filename = "example.ts",
                                                            className = ""
                                                          }) => {
  return (
    <div className={`mb-64 relative ${className}`}>
      <div className="absolute -inset-1 bg-gradient-to-r rounded-3xl opacity-20
        blur-2xl"></div>
      <div className="relative glass-card rounded-3xl border border-white/10
        overflow-hidden shadow-2xl">
        <div className="flex items-center gap-2 px-6 py-4 border-b border-white/5 
        bg-white/5">
          <div className="flex gap-2">
            <div className="w-3 h-3 rounded-full bg-red-500/20 border border-red-500/5"></div>
            <div className="w-3 h-3 rounded-full bg-yellow-500/20 border border-yellow-500/5"></div>
            <div className="w-3 h-3 rounded-full bg-green-500/20 border border-green-500/5"></div>
          </div>
          <div className="ml-4 text-xs font-mono text-white/40">{filename}</div>
        </div>
        <div className="p-8 overflow-x-auto">
            <pre className="font-mono text-sm md:text-base leading-relaxed text-white/80">
            <code dangerouslySetInnerHTML={{
              __html: code
            }}/>
            </pre>
        </div>
      </div>
    </div>
  );
};
