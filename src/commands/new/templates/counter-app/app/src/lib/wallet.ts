import { Address, fromNano } from '@ton/core';
import { useQuery } from '@tanstack/react-query';

import { TON_NETWORK_MODE, tonClient } from './ton';

export function useWalletBalance(address: string | undefined) {
  return useQuery({
    queryKey: ['wallet-balance', TON_NETWORK_MODE, address ?? null],
    queryFn: async () => {
      if (!address) return null;
      const balance = await tonClient.getBalance(Address.parse(address));
      return fromNano(balance);
    },
    enabled: Boolean(address),
    refetchInterval: 15_000,
  });
}
