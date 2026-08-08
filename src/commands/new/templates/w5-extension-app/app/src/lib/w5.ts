import { Buffer } from 'buffer';
import { Address } from '@ton/core';

import {
  CollectPaymentFromWallet,
  SimpleExtension,
  WithdrawExtensionBalance,
} from '@wrappers/SimpleExtension.gen';
import { WalletV5 } from '@wrappers/WalletV5.gen';
import {
  formatAddressForNetwork,
  getErrorMessage,
  getExplorerUrl,
  normalizeAddress,
  runToncenterRequest,
  toRawAddress,
  tonClient,
} from './ton';

type ContractStateName = 'active' | 'uninitialized' | 'frozen';

const UINT64_MAX = (1n << 64n) - 1n;

export type ExtensionExternalAction =
  | 'collectPaymentFromWallet'
  | 'withdrawExtensionBalance';

export const EXTENSION_EXTERNAL_ACTION_LABELS: Record<
  ExtensionExternalAction,
  string
> = {
  collectPaymentFromWallet: 'Collect payment',
  withdrawExtensionBalance: 'Withdraw balance',
};

export class MissingGetMethodError extends Error {
  constructor(
    readonly method: string,
    readonly address: string,
  ) {
    super(
      `${method} is not available at ${address}. TVM exit code -11 means the contract does not expose this get method.`,
    );
    this.name = 'MissingGetMethodError';
  }
}

export interface ExtensionDetails {
  walletAddress: string;
  admin: string;
  subscriptionAmount: bigint;
  lastPaymentTime: bigint | null;
  paymentTimeInterval: bigint;
}

export interface ExtensionInspection {
  address: string;
  rawAddress: string;
  explorerUrl: string;
  state: ContractStateName | 'unknown';
  balance: bigint | null;
  kind: 'simple-extension' | 'missing-getter' | 'lookup-error' | 'inactive';
  kindLabel: string;
  infoStatus: 'ready' | 'missing-getter' | 'error' | 'inactive';
  info: ExtensionDetails | null;
  errorMessage: string | null;
}

export interface WalletInspection {
  address: string;
  rawAddress: string;
  explorerUrl: string;
  balance: bigint | null;
  skippedSelfReferenceCount: number;
  extensions: ExtensionInspection[];
}

export function getTvmExitCode(error: unknown): number | null {
  const message = getErrorMessage(error);
  const match =
    message.match(/exit[_\s-]?code["']?\s*[:=]?\s*(-?\d+)/i) ??
    message.match(/exit\s+code\s*(-?\d+)/i);

  return match ? Number(match[1]) : null;
}

export function isMissingGetMethodError(error: unknown): boolean {
  return (
    error instanceof MissingGetMethodError || getTvmExitCode(error) === -11
  );
}

function hashToAddress(workchain: number, hash: bigint): Address {
  const hashBuffer = Buffer.from(hash.toString(16).padStart(64, '0'), 'hex');
  return new Address(workchain, hashBuffer);
}

function publicError(prefix: string, error: unknown): string {
  return `${prefix} ${getErrorMessage(error)}`;
}

function parseUint64(value: string | bigint | number, label: string): bigint {
  const parsed =
    typeof value === 'bigint'
      ? value
      : typeof value === 'number'
        ? BigInt(value)
        : BigInt(value.trim());

  if (parsed < 0n || parsed > UINT64_MAX) {
    throw new Error(`${label} must fit into uint64.`);
  }

  return parsed;
}

function nextQueryId(): bigint {
  return BigInt(Date.now());
}

function createExternalBody(options: {
  action: ExtensionExternalAction;
  queryId?: string | bigint | number;
}) {
  if (options.action === 'collectPaymentFromWallet') {
    return CollectPaymentFromWallet.toCell(
      CollectPaymentFromWallet.create({
        queryId:
          options.queryId === undefined
            ? nextQueryId()
            : parseUint64(options.queryId, 'Query ID'),
      }),
    );
  }

  return WithdrawExtensionBalance.toCell(WithdrawExtensionBalance.create());
}

async function inspectExtensionsSequentially(
  extensionAddresses: Address[],
): Promise<ExtensionInspection[]> {
  const inspections: ExtensionInspection[] = [];

  for (const extensionAddress of extensionAddresses) {
    inspections.push(await inspectExtension(extensionAddress));
  }

  return inspections;
}

async function inspectExtension(
  extensionAddress: Address,
): Promise<ExtensionInspection> {
  const address = formatAddressForNetwork(extensionAddress);
  const base = {
    address,
    rawAddress: toRawAddress(extensionAddress),
    explorerUrl: getExplorerUrl(extensionAddress),
    state: 'unknown' as const,
    balance: null,
  };

  try {
    const contract = tonClient.open(
      SimpleExtension.fromAddress(extensionAddress),
    );
    const info = await runToncenterRequest(() => contract.getExtensionInfo());
    const state = await runToncenterRequest(() =>
      tonClient.getContractState(extensionAddress),
    ).catch(() => null);

    return {
      ...base,
      state: state?.state ?? base.state,
      balance: state?.balance ?? base.balance,
      kind: 'simple-extension',
      kindLabel: 'SimpleExtension',
      infoStatus: 'ready',
      info: {
        walletAddress: formatAddressForNetwork(info.walletAddress),
        admin: formatAddressForNetwork(info.admin),
        subscriptionAmount: info.subscriptionAmount,
        lastPaymentTime: info.lastPaymentTime,
        paymentTimeInterval: info.paymentTimeInterval,
      },
      errorMessage: null,
    };
  } catch (error) {
    if (isMissingGetMethodError(error)) {
      return {
        ...base,
        kind: 'missing-getter',
        kindLabel: 'No extensionInfo',
        infoStatus: 'missing-getter',
        info: null,
        errorMessage:
          'This installed contract does not expose extensionInfo(). TVM exit code -11 means the get method is missing.',
      };
    }

    return {
      ...base,
      kind: 'lookup-error',
      kindLabel: 'Lookup failed',
      infoStatus: 'error',
      info: null,
      errorMessage: publicError('Unable to read extensionInfo().', error),
    };
  }
}

export async function inspectWallet(
  walletAddressValue: string,
): Promise<WalletInspection> {
  const walletAddress = Address.parse(walletAddressValue);
  const normalizedAddress = normalizeAddress(walletAddress);
  const wallet = tonClient.open(WalletV5.fromAddress(walletAddress));
  let extensionsDict: Awaited<ReturnType<typeof wallet.getExtensions>>;

  try {
    extensionsDict = await runToncenterRequest(() => wallet.getExtensions());
  } catch (error) {
    if (isMissingGetMethodError(error)) {
      throw new MissingGetMethodError('get_extensions', normalizedAddress);
    }

    throw new Error(
      publicError(
        `Failed to call get_extensions on ${normalizedAddress}.`,
        error,
      ),
    );
  }

  const discoveredAddresses = extensionsDict
    .keys()
    .map((hash) => hashToAddress(walletAddress.workChain, hash))
    .sort((left, right) =>
      left.toRawString().localeCompare(right.toRawString()),
    );
  const extensionAddresses = discoveredAddresses.filter(
    (extensionAddress) => !extensionAddress.equals(walletAddress),
  );
  const extensions = await inspectExtensionsSequentially(extensionAddresses);

  return {
    address: normalizedAddress,
    rawAddress: toRawAddress(walletAddress),
    explorerUrl: getExplorerUrl(walletAddress),
    balance: null,
    skippedSelfReferenceCount:
      discoveredAddresses.length - extensionAddresses.length,
    extensions,
  };
}

export async function sendExtensionExternalMessage(options: {
  extensionAddress: string;
  action: ExtensionExternalAction;
  queryId?: string | bigint | number;
}): Promise<{ address: string; rawAddress: string; explorerUrl: string }> {
  const extensionAddress = Address.parse(options.extensionAddress);
  const body = createExternalBody(options);

  await runToncenterRequest(() =>
    tonClient.provider(extensionAddress).external(body),
  );

  return {
    address: formatAddressForNetwork(extensionAddress),
    rawAddress: toRawAddress(extensionAddress),
    explorerUrl: getExplorerUrl(extensionAddress),
  };
}
