import { Check, ChevronDown } from 'lucide-react';

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import type { Network } from '../lib/router';

function StatusDot({ network }: { network: 'mainnet' | 'testnet' }) {
  return (
    <svg className="size-2 fill-current" viewBox="0 0 8 8">
      <circle
        cx="4"
        cy="4"
        r="4"
        className={network === 'mainnet' ? 'text-success' : 'text-warning'}
      />
    </svg>
  );
}

export function NetworkDropdown({
  network,
  setTestnet,
}: {
  network: Network;
  setTestnet: (testnet: boolean) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          className="rounded-full h-10 px-3 gap-1.5 text-[15px] font-bold bg-secondary max-sm:h-9 max-sm:text-sm max-sm:px-2.5"
        >
          <StatusDot network={network} />
          {network === 'mainnet' ? 'Mainnet' : 'Testnet'}
          <ChevronDown className="size-3 opacity-50" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-[180px] rounded-xl p-2">
        <DropdownMenuItem
          className="rounded-xl px-3.5 py-3 text-[15px] font-medium gap-2.5 cursor-pointer"
          onClick={() => setTestnet(false)}
        >
          <StatusDot network="mainnet" />
          Mainnet
          {network === 'mainnet' && <Check className="size-4 ml-auto" />}
        </DropdownMenuItem>
        <DropdownMenuItem
          className="rounded-xl px-3.5 py-3 text-[15px] font-medium gap-2.5 cursor-pointer"
          onClick={() => setTestnet(true)}
        >
          <StatusDot network="testnet" />
          Testnet
          {network === 'testnet' && <Check className="size-4 ml-auto" />}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
