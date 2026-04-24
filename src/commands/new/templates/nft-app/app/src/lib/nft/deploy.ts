import { Dictionary, toNano } from '@ton/core';
import type { SendTransactionRequest } from '@tonconnect/ui-react';

import {
  BatchDeployDictItem,
  CollectionContent,
  NftCollection,
  NftItemInitAtDeployment,
  RoyaltyParams,
} from '@wrappers/NftCollection.gen';
import { NftItem } from '@wrappers/NftItem.gen';

import { networkChain } from '../ton';
import type { Network } from '../router';
import { formatAddr, parseAddr } from './address';
import { DEFAULTS } from './defaults';
import { encodeStateInit, toB64, txValidUntil } from './internal';
import {
  buildOffchainMetadataCell,
  buildOnchainMetadataCell,
} from './metadata';

export interface DeployCollectionArgs {
  network: Network;
  admin: string;
  metadata:
    | { kind: 'onchain'; name: string; description: string; image: string }
    | { kind: 'offchain'; uri: string };
  commonContent: string;
  royaltyAddress?: string;
  royaltyNumerator?: bigint;
  royaltyDenominator?: bigint;
  deployAmount?: string;
}

export function buildDeployCollectionTx(args: DeployCollectionArgs): {
  address: string;
  request: SendTransactionRequest;
} {
  const adminAddr = parseAddr(args.admin);
  const royaltyAddr = parseAddr(args.royaltyAddress ?? args.admin);
  const metadataCell =
    args.metadata.kind === 'onchain'
      ? buildOnchainMetadataCell({
          name: args.metadata.name,
          description: args.metadata.description,
          image: args.metadata.image,
        })
      : buildOffchainMetadataCell(args.metadata.uri);
  const contract = NftCollection.fromStorage({
    adminAddress: adminAddr,
    nextItemIndex: 0n,
    content: {
      ref: CollectionContent.create({
        collectionMetadata: metadataCell,
        commonContent: args.commonContent,
      }),
    },
    nftItemCode: NftItem.CodeCell,
    royaltyParams: {
      ref: RoyaltyParams.create({
        numerator: args.royaltyNumerator ?? DEFAULTS.royaltyNumerator,
        denominator: args.royaltyDenominator ?? DEFAULTS.royaltyDenominator,
        royaltyAddress: royaltyAddr,
      }),
    },
  });

  const address = formatAddr(contract.address, args.network);
  return {
    address,
    request: {
      validUntil: txValidUntil(),
      network: networkChain(args.network),
      messages: [
        {
          address,
          amount: toNano(
            args.deployAmount ?? DEFAULTS.deployCollectionValue,
          ).toString(),
          stateInit: encodeStateInit(contract),
        },
      ],
    },
  };
}

export interface DeployItemArgs {
  network: Network;
  collectionAddress: string;
  itemIndex: bigint;
  itemOwner: string;
  itemContent: string;
  attachTonAmount?: string;
  msgValue?: string;
}

export function buildDeployItemTx(
  args: DeployItemArgs,
): SendTransactionRequest {
  const collectionAddr = parseAddr(args.collectionAddress);
  const ownerAddr = parseAddr(args.itemOwner);
  const payload = NftCollection.createCellOfDeployNft({
    queryId: BigInt(Date.now()),
    itemIndex: args.itemIndex,
    attachTonAmount: toNano(args.attachTonAmount ?? DEFAULTS.perItemTonAmount),
    initParams: {
      ref: NftItemInitAtDeployment.create({
        ownerAddress: ownerAddr,
        content: args.itemContent,
      }),
    },
  });

  return {
    validUntil: txValidUntil(),
    network: networkChain(args.network),
    messages: [
      {
        address: formatAddr(collectionAddr, args.network),
        amount: toNano(args.msgValue ?? DEFAULTS.deployItemValue).toString(),
        payload: toB64(payload),
      },
    ],
  };
}

export interface BatchItem {
  index: bigint;
  owner: string;
  content: string;
  attachTonAmount?: string;
}

export function buildBatchDeployTx(
  network: Network,
  collectionAddress: string,
  items: BatchItem[],
  msgValue?: string,
): SendTransactionRequest {
  const collectionAddr = parseAddr(collectionAddress);
  const dict = Dictionary.empty<bigint, BatchDeployDictItem>(
    Dictionary.Keys.BigUint(64),
    {
      serialize: (v, b) => BatchDeployDictItem.store(v, b),
      parse: (s) => BatchDeployDictItem.fromSlice(s),
    },
  );
  for (const it of items) {
    dict.set(
      it.index,
      BatchDeployDictItem.create({
        attachTonAmount: toNano(
          it.attachTonAmount ?? DEFAULTS.perItemTonAmount,
        ),
        initParams: {
          ref: NftItemInitAtDeployment.create({
            ownerAddress: parseAddr(it.owner),
            content: it.content,
          }),
        },
      }),
    );
  }

  const payload = NftCollection.createCellOfBatchDeployNfts({
    queryId: BigInt(Date.now()),
    deployList: dict,
  });

  const totalAmount =
    toNano(msgValue ?? DEFAULTS.batchExtraValue) +
    items.reduce(
      (acc, it) =>
        acc + toNano(it.attachTonAmount ?? DEFAULTS.perItemTonAmount),
      0n,
    );

  return {
    validUntil: txValidUntil(),
    network: networkChain(network),
    messages: [
      {
        address: formatAddr(collectionAddr, network),
        amount: totalAmount.toString(),
        payload: toB64(payload),
      },
    ],
  };
}
