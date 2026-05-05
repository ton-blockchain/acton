import { Address, beginCell, storeStateInit, toNano } from '@ton/core';
import type { SendTransactionRequest } from '@tonconnect/ui-react';

import { Counter } from '../../../wrappers-ts/Counter.gen';
import { IS_TESTNET, TON_CHAIN, tonClient } from './ton';

const UINT32_MAX = 4_294_967_295n;
const TRANSACTION_TTL_SECONDS = 5 * 60;

export const DEFAULT_COUNTER_ID = '1';
export const DEFAULT_STEP = '1';
export const DEFAULT_DEPLOY_VALUE = '0.05';
export const DEFAULT_MESSAGE_VALUE = '0.02';

export interface CounterPreview {
  address: string;
  contract: Counter;
  id: bigint;
  owner: string;
}

export interface CounterSnapshot {
  address: string;
  isDeployed: boolean;
  owner: string | null;
  value: bigint | null;
}

export type CounterAction = 'increase' | 'decrease';

function parseUint32(
  value: string,
  label: string,
  options?: { allowZero?: boolean },
): bigint {
  const trimmed = value.trim();

  if (!/^\d+$/.test(trimmed)) {
    throw new Error(`${label} must be a whole number.`);
  }

  const parsed = BigInt(trimmed);
  const allowZero = options?.allowZero ?? true;

  if (!allowZero && parsed === 0n) {
    throw new Error(`${label} must be greater than 0.`);
  }

  if (parsed > UINT32_MAX) {
    throw new Error(`${label} must fit into uint32.`);
  }

  return parsed;
}

function parseTonAmount(value: string, label: string): bigint {
  const trimmed = value.trim();

  if (!trimmed) {
    throw new Error(`${label} is required.`);
  }

  return toNano(trimmed);
}

function encodeStateInit(contract: Counter): string {
  if (!contract.init) {
    throw new Error('Counter init is missing.');
  }

  return beginCell()
    .store(storeStateInit(contract.init))
    .endCell()
    .toBoc()
    .toString('base64');
}

function encodePayload(action: CounterAction, step: bigint): string {
  const body =
    action === 'increase'
      ? Counter.createCellOfIncreaseCounter({ increaseBy: step })
      : Counter.createCellOfDecreaseCounter({ decreaseBy: step });

  return body.toBoc().toString('base64');
}

function transactionExpiry(): number {
  return Math.floor(Date.now() / 1000) + TRANSACTION_TTL_SECONDS;
}

export function formatAddress(address: Address): string {
  return address.toString({
    bounceable: false,
    testOnly: IS_TESTNET,
  });
}

export function normalizeCounterAddress(address: string): string {
  return formatAddress(Address.parse(address));
}

export function getCounterPreview(
  counterIdValue: string,
  ownerAddressValue: string,
): CounterPreview {
  const id = parseUint32(counterIdValue, 'Counter ID');
  const owner = Address.parse(ownerAddressValue);
  const contract = Counter.fromStorage({ id, owner, counter: 0n });

  return {
    address: formatAddress(contract.address),
    contract,
    id,
    owner: formatAddress(owner),
  };
}

export async function isCounterDeployed(address: Address): Promise<boolean> {
  return tonClient.isContractDeployed(address);
}

export async function readCounter(
  addressValue: string,
): Promise<CounterSnapshot> {
  const address = Address.parse(addressValue);
  const normalizedAddress = formatAddress(address);
  const isDeployed = await tonClient.isContractDeployed(address);

  if (!isDeployed) {
    return {
      address: normalizedAddress,
      isDeployed: false,
      owner: null,
      value: null,
    };
  }

  const contract = tonClient.open(Counter.fromAddress(address));
  const owner = await contract.getOwner();
  const value = await contract.getCurrentCounter();

  return {
    address: normalizedAddress,
    isDeployed: true,
    owner: formatAddress(owner),
    value,
  };
}

export function buildDeployTransaction(
  counterIdValue: string,
  deployAmountValue: string,
  ownerAddressValue: string,
): {
  address: string;
  request: SendTransactionRequest;
  preview: CounterPreview;
} {
  const preview = getCounterPreview(counterIdValue, ownerAddressValue);
  const amount = parseTonAmount(deployAmountValue, 'Deploy value');

  return {
    address: preview.address,
    preview,
    request: {
      network: TON_CHAIN,
      validUntil: transactionExpiry(),
      messages: [
        {
          address: preview.address,
          amount: amount.toString(),
          stateInit: encodeStateInit(preview.contract),
        },
      ],
    },
  };
}

export function buildCounterActionTransaction(options: {
  action: CounterAction;
  addressValue: string;
  messageValue: string;
  stepValue: string;
}): { address: string; request: SendTransactionRequest } {
  const address = Address.parse(options.addressValue);
  const normalizedAddress = formatAddress(address);
  const amount = parseTonAmount(options.messageValue, 'Message value');
  const step = parseUint32(options.stepValue, 'Step', { allowZero: false });

  return {
    address: normalizedAddress,
    request: {
      network: TON_CHAIN,
      validUntil: transactionExpiry(),
      messages: [
        {
          address: normalizedAddress,
          amount: amount.toString(),
          payload: encodePayload(options.action, step),
        },
      ],
    },
  };
}

export function getErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null) {
    const errorWithStatus = error as {
      message?: string;
      response?: { status?: number };
      status?: number;
    };
    const status =
      errorWithStatus.response?.status ?? errorWithStatus.status ?? null;

    if (
      status === 429 ||
      errorWithStatus.message?.includes('status code 429')
    ) {
      return 'Toncenter rate limit reached (HTTP 429). This app reads chain data through Toncenter, so wait a bit and try again, or add TONCENTER_TESTNET_API_KEY / TONCENTER_MAINNET_API_KEY for higher limits.';
    }
  }

  if (error instanceof Error) {
    return error.message;
  }

  return 'Unexpected error.';
}
