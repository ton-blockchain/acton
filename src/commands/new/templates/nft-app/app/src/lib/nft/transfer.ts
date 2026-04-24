import { beginCell, toNano } from '@ton/core';
import type { SendTransactionRequest } from '@tonconnect/ui-react';

import { NftCollection } from '@wrappers/NftCollection.gen';
import { NftItem } from '@wrappers/NftItem.gen';

import { networkChain } from '../ton';
import type { Network } from '../router';
import { formatAddr, parseAddr } from './address';
import { DEFAULTS } from './defaults';
import { toB64, txValidUntil } from './internal';

export function buildTransferItemTx(
  network: Network,
  itemAddress: string,
  newOwner: string,
  options?: { forwardTonAmount?: string; value?: string },
): SendTransactionRequest {
  const itemAddr = parseAddr(itemAddress);
  const newOwnerAddr = parseAddr(newOwner);
  const payload = NftItem.createCellOfAskToChangeOwnership({
    queryId: BigInt(Date.now()),
    newOwnerAddress: newOwnerAddr,
    sendExcessesTo: null,
    customPayload: null,
    forwardTonAmount: toNano(
      options?.forwardTonAmount ?? DEFAULTS.transferForwardAmount,
    ),
    forwardPayload: {
      $: 'PayloadInline',
      value: beginCell().endCell().beginParse(),
    },
  });
  return {
    validUntil: txValidUntil(),
    network: networkChain(network),
    messages: [
      {
        address: formatAddr(itemAddr, network),
        amount: toNano(options?.value ?? DEFAULTS.transferValue).toString(),
        payload: toB64(payload),
      },
    ],
  };
}

export function buildChangeAdminTx(
  network: Network,
  collectionAddress: string,
  newAdmin: string,
): SendTransactionRequest {
  const newAdminAddr = parseAddr(newAdmin);
  const payload = NftCollection.createCellOfChangeCollectionAdmin({
    queryId: BigInt(Date.now()),
    newAdminAddress: newAdminAddr,
  });
  return {
    validUntil: txValidUntil(),
    network: networkChain(network),
    messages: [
      {
        address: formatAddr(parseAddr(collectionAddress), network),
        amount: toNano(DEFAULTS.changeAdminValue).toString(),
        payload: toB64(payload),
      },
    ],
  };
}
