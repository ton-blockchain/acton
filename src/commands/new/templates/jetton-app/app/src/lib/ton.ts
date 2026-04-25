import { TonClient } from '@ton/ton';
import { Address, beginCell } from '@ton/core';

type Network = 'mainnet' | 'testnet';

const clients: Record<string, TonClient> = {};

function toncenterApiKey(network: Network): string | undefined {
  return network === 'mainnet'
    ? import.meta.env.TONCENTER_MAINNET_API_KEY
    : import.meta.env.TONCENTER_TESTNET_API_KEY;
}

function toncenterApiHeaders(network: Network): HeadersInit | undefined {
  const apiKey = toncenterApiKey(network);
  return apiKey ? { 'X-API-Key': apiKey } : undefined;
}

export function getTonClient(network: Network): TonClient {
  if (!clients[network]) {
    const endpoint =
      network === 'mainnet'
        ? 'https://toncenter.com/api/v2/jsonRPC'
        : 'https://testnet.toncenter.com/api/v2/jsonRPC';
    clients[network] = new TonClient({ endpoint, apiKey: toncenterApiKey(network) });
  }
  return clients[network]!;
}

export async function getWalletAddress(
  client: TonClient,
  minterAddress: Address,
  ownerAddress: Address,
): Promise<Address> {
  const result = await client.runMethod(minterAddress, 'get_wallet_address', [
    {
      type: 'slice',
      cell: beginCell().storeAddress(ownerAddress).endCell(),
    },
  ]);
  return result.stack.readAddress();
}

export interface JettonMasterInfo {
  totalSupply: bigint;
  mintable: boolean;
  adminAddress: Address | null;
  metadata: {
    name?: string;
    symbol?: string;
    decimals?: string;
    description?: string;
    image?: string;
  };
}

const toncenterV3 = {
  mainnet: 'https://toncenter.com/api/v3',
  testnet: 'https://testnet.toncenter.com/api/v3',
};

async function fetchWithRetry(
  url: string,
  init?: RequestInit,
  maxRetries = 4,
): Promise<Response> {
  let delay = 1000;
  for (let i = 0; i <= maxRetries; i++) {
    const res = await fetch(url, init);
    if (res.status === 429 && i < maxRetries) {
      await new Promise((r) => setTimeout(r, delay));
      delay *= 2;
      continue;
    }
    return res;
  }
  throw new Error('Max retries exceeded');
}

export async function fetchJettonMaster(
  network: Network,
  address: string,
): Promise<JettonMasterInfo> {
  const base = toncenterV3[network];
  const res = await fetchWithRetry(
    `${base}/jetton/masters?address=${encodeURIComponent(address)}&limit=1&offset=0`,
    { headers: toncenterApiHeaders(network) },
  );
  if (!res.ok) throw new Error(`Toncenter API error: ${res.status}`);

  const json = await res.json();
  const masters = json.jetton_masters;
  if (!masters || masters.length === 0) {
    throw new Error('Jetton not found');
  }

  const master = masters[0];
  const rawAddr = master.address as string;

  const metaEntry = json.metadata?.[rawAddr]?.token_info?.[0];

  let adminAddr: Address | null = null;
  try {
    if (master.admin_address) {
      adminAddr = Address.parse(master.admin_address);
    }
  } catch {
    /* addr_none */
  }

  return {
    totalSupply: BigInt(master.total_supply),
    mintable: master.mintable,
    adminAddress: adminAddr,
    metadata: {
      name: metaEntry?.name || undefined,
      symbol: metaEntry?.symbol || undefined,
      decimals:
        metaEntry?.extra?.decimals || master.jetton_content?.decimals || undefined,
      description: metaEntry?.description || undefined,
      image: metaEntry?.image || undefined,
    },
  };
}
