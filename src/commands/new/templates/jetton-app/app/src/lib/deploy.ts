import { Address, beginCell, Cell, toNano } from '@ton/core';
import {
  JettonMinter,
  MinterStorage,
  MintNewJettons,
  InternalTransferStep,
  ChangeMinterAdmin,
  ChangeMinterMetadata,
} from '@wrappers/JettonMinter.gen';
import {
  JettonWallet,
  AskToBurn,
  AskToTransfer,
} from '@wrappers/JettonWallet.gen';
import { buildOnchainMetadata, type JettonMetadata } from './jettonContent';

export function parseUnits(amount: string, decimals: number): bigint {
  const [whole = '', fracRaw = ''] = amount.split('.');
  const frac = fracRaw.slice(0, decimals).padEnd(decimals, '0');
  return BigInt(whole + frac);
}

export async function buildDeployMessage(params: {
  metadata: JettonMetadata;
  ownerAddress: Address;
  mintAmount: bigint;
}) {
  const content = await buildOnchainMetadata(params.metadata);

  const minter = JettonMinter.fromStorage({
    totalSupply: 0n,
    adminAddress: params.ownerAddress,
    nextAdminAddress: null,
    metadata: content,
  });

  const mintBody = buildMintBody({
    toAddress: params.ownerAddress,
    jettonAmount: params.mintAmount,
    forwardTonAmount: toNano('0.02'),
    totalTonAmount: toNano('0.05'),
  });

  return {
    contractAddress: minter.address,
    stateInit: minter.init!,
    mintBody,
  };
}

export function buildMintBody(params: {
  toAddress: Address;
  jettonAmount: bigint;
  forwardTonAmount: bigint;
  totalTonAmount: bigint;
  queryId?: bigint;
}): Cell {
  const {
    toAddress,
    jettonAmount,
    forwardTonAmount,
    totalTonAmount,
    queryId = 0n,
  } = params;

  return MintNewJettons.toCell(
    MintNewJettons.create({
      queryId,
      mintRecipient: toAddress,
      tonAmount: totalTonAmount,
      internalTransferMsg: {
        ref: InternalTransferStep.create({
          queryId,
          jettonAmount,
          transferInitiator: null,
          sendExcessesTo: null,
          forwardTonAmount,
          forwardPayload: beginCell().storeUint(0, 1).asSlice(),
        }),
      },
    }),
  );
}

export function buildChangeAdminBody(newAdmin: Address, queryId = 0n): Cell {
  return ChangeMinterAdmin.toCell(
    ChangeMinterAdmin.create({ queryId, newAdminAddress: newAdmin }),
  );
}

export async function buildChangeContentBody(
  metadata: JettonMetadata,
  queryId = 0n,
): Promise<Cell> {
  const content = await buildOnchainMetadata(metadata);
  return ChangeMinterMetadata.toCell(
    ChangeMinterMetadata.create({ queryId, newMetadata: content }),
  );
}

export function buildBurnBody(
  amount: bigint,
  responseAddress: Address,
  queryId = 0n,
): Cell {
  return AskToBurn.toCell(
    AskToBurn.create({
      queryId,
      jettonAmount: amount,
      sendExcessesTo: responseAddress,
      customPayload: null,
    }),
  );
}

export function buildTransferBody(params: {
  toAddress: Address;
  amount: bigint;
  responseAddress: Address;
  forwardTonAmount?: bigint;
  forwardPayload?: Cell | null;
  queryId?: bigint;
}): Cell {
  const {
    toAddress,
    amount,
    responseAddress,
    forwardTonAmount = 0n,
    forwardPayload = null,
    queryId = 0n,
  } = params;

  const payloadSlice = forwardPayload
    ? beginCell()
        .storeUint(1, 1)
        .storeRef(forwardPayload)
        .endCell()
        .beginParse()
    : beginCell().storeUint(0, 1).asSlice();

  return AskToTransfer.toCell(
    AskToTransfer.create({
      queryId,
      jettonAmount: amount,
      transferRecipient: toAddress,
      sendExcessesTo: responseAddress,
      customPayload: null,
      forwardTonAmount,
      forwardPayload: payloadSlice,
    }),
  );
}
