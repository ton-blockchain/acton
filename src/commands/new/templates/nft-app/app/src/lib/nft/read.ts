import { Address, Cell } from '@ton/core';

import {
  NftCollection,
  NftCollectionStorage,
} from '@wrappers/NftCollection.gen';

import { getTonClient } from '../ton';
import type { Network } from '../router';
import { formatAddr, formatAddrNonBounce, parseAddr } from './address';
import { decodeCollectionMetadata, type CollectionMetadata } from './metadata';

export interface CollectionOnChain {
  address: string;
  nextItemIndex: number;
  adminAddress: string;
  commonContent: string;
  royaltyNumerator: number;
  royaltyDenominator: number;
  royaltyAddress: string;
  metadata: CollectionMetadata;
}

export async function readCollection(
  network: Network,
  addressValue: string,
): Promise<CollectionOnChain | null> {
  const client = getTonClient(network);
  const addr = parseAddr(addressValue);
  const state = await client.getContractState(addr);
  if (state.state !== 'active' || !state.data) return null;

  // Parse the full storage cell to get commonContent (get_collection_data
  // doesn't expose it) alongside the usual fields.
  let commonContent = '';
  let nextItemIndex = 0;
  let adminAddress: Address | null = null;
  let royaltyNumerator = 0;
  let royaltyDenominator = 100;
  let royaltyAddress: Address | null = null;
  let metadata: CollectionMetadata = {};
  try {
    const storage = NftCollectionStorage.fromSlice(
      Cell.fromBoc(state.data)[0].beginParse(),
    );
    nextItemIndex = Number(storage.nextItemIndex);
    adminAddress = storage.adminAddress;
    commonContent = storage.content.ref.commonContent;
    royaltyNumerator = Number(storage.royaltyParams.ref.numerator);
    royaltyDenominator = Number(storage.royaltyParams.ref.denominator);
    royaltyAddress = storage.royaltyParams.ref.royaltyAddress;
    metadata = decodeCollectionMetadata(storage.content.ref.collectionMetadata);
  } catch {
    // Fallback: older layout or partial read — use get-method results
    const contract = client.open(NftCollection.fromAddress(addr));
    const data = await contract.getCollectionData();
    nextItemIndex = Number(data.nextItemIndex);
    adminAddress = data.adminAddress;
    metadata = decodeCollectionMetadata(data.collectionMetadata);
    const royalty = await contract.getRoyaltyParams().catch(() => null);
    if (royalty) {
      royaltyNumerator = Number(royalty.numerator);
      royaltyDenominator = Number(royalty.denominator);
      royaltyAddress = royalty.royaltyAddress;
    }
  }

  return {
    address: formatAddr(addr, network),
    nextItemIndex,
    adminAddress: adminAddress
      ? formatAddrNonBounce(adminAddress, network)
      : '',
    commonContent,
    royaltyNumerator,
    royaltyDenominator,
    royaltyAddress: royaltyAddress
      ? formatAddrNonBounce(royaltyAddress, network)
      : '',
    metadata,
  };
}

export async function getNftAddressByIndex(
  network: Network,
  collectionAddress: string,
  index: bigint,
): Promise<string> {
  const client = getTonClient(network);
  const contract = client.open(
    NftCollection.fromAddress(parseAddr(collectionAddress)),
  );
  const addr = await contract.getNftAddressByIndex(index);
  return formatAddr(addr, network);
}
