import { Address } from '@ton/core';

import type { Network } from '../router';

export const ZERO_ADDRESS = 'UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACgQ';

export function formatAddr(a: Address, network: Network): string {
  return a.toString({ bounceable: true, testOnly: network === 'testnet' });
}

export function formatAddrNonBounce(a: Address, network: Network): string {
  return a.toString({ bounceable: false, testOnly: network === 'testnet' });
}

export function parseAddr(v: string): Address {
  return Address.parse(v.trim());
}
