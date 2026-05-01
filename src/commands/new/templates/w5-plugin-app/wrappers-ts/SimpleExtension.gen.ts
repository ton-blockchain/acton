// AUTO-GENERATED, do not edit
// it's a TypeScript wrapper for a SimpleExtension contract in Tolk
/* eslint-disable */

import * as c from '@ton/core';
import { beginCell, ContractProvider, Sender, SendMode } from '@ton/core';

// ————————————————————————————————————————————
//   predefined types and functions
//

type RemainingBitsAndRefs = c.Slice;

type StoreCallback<T> = (obj: T, b: c.Builder) => void;
type LoadCallback<T> = (s: c.Slice) => T;

export type CellRef<T> = {
  ref: T;
};

function makeCellFrom<T>(self: T, storeFn_T: StoreCallback<T>): c.Cell {
  let b = beginCell();
  storeFn_T(self, b);
  return b.endCell();
}

function loadAndCheckPrefix32(
  s: c.Slice,
  expected: number,
  structName: string,
): void {
  let prefix = s.loadUint(32);
  if (prefix !== expected) {
    throw new Error(
      `Incorrect prefix for '${structName}': expected 0x${expected.toString(16).padStart(8, '0')}, got 0x${prefix.toString(16).padStart(8, '0')}`,
    );
  }
}

function lookupPrefix(
  s: c.Slice,
  expected: number,
  prefixLen: number,
): boolean {
  return s.remainingBits >= prefixLen && s.preloadUint(prefixLen) === expected;
}

function throwNonePrefixMatch(fieldPath: string): never {
  throw new Error(
    `Incorrect prefix for '${fieldPath}': none of variants matched`,
  );
}

function storeCellRef<T>(
  cell: CellRef<T>,
  b: c.Builder,
  storeFn_T: StoreCallback<T>,
): void {
  let b_ref = c.beginCell();
  storeFn_T(cell.ref, b_ref);
  b.storeRef(b_ref.endCell());
}

function loadCellRef<T>(s: c.Slice, loadFn_T: LoadCallback<T>): CellRef<T> {
  let s_ref = s.loadRef().beginParse();
  return { ref: loadFn_T(s_ref) };
}

function storeTolkRemaining(v: RemainingBitsAndRefs, b: c.Builder): void {
  b.storeSlice(v);
}

function loadTolkRemaining(s: c.Slice): RemainingBitsAndRefs {
  let rest = s.clone();
  s.loadBits(s.remainingBits);
  while (s.remainingRefs) {
    s.loadRef();
  }
  return rest;
}

function storeTolkNullable<T>(
  v: T | null,
  b: c.Builder,
  storeFn_T: StoreCallback<T>,
): void {
  if (v === null) {
    b.storeUint(0, 1);
  } else {
    b.storeUint(1, 1);
    storeFn_T(v, b);
  }
}

// ————————————————————————————————————————————
//   parse get methods result from a TVM stack
//

class StackReader {
  constructor(private tuple: c.TupleItem[]) {}

  static fromGetMethod(
    expectedN: number,
    getMethodResult: { stack: c.TupleReader },
  ): StackReader {
    let tuple = [] as c.TupleItem[];
    while (getMethodResult.stack.remaining) {
      tuple.push(getMethodResult.stack.pop());
    }
    if (tuple.length !== expectedN) {
      throw new Error(`expected ${expectedN} stack width, got ${tuple.length}`);
    }
    return new StackReader(tuple);
  }

  private popExpecting<ItemT>(itemType: string): ItemT {
    const item = this.tuple.shift();
    if (item?.type !== itemType) {
      throw new Error(`not '${itemType}' on a stack`);
    }
    return item as ItemT;
  }

  readBigInt(): bigint {
    return this.popExpecting<c.TupleItemInt>('int').value;
  }

  readBoolean(): boolean {
    return this.popExpecting<c.TupleItemInt>('int').value !== 0n;
  }

  readCell(): c.Cell {
    const item = this.tuple.shift();
    if (
      item?.type !== 'cell' &&
      item?.type !== 'slice' &&
      item?.type !== 'builder'
    ) {
      throw new Error(`not a cell-like value on a stack`);
    }
    return (item as c.TupleItemCell | c.TupleItemSlice | c.TupleItemBuilder)
      .cell;
  }

  readAddress(): c.Address {
    return this.readCell().beginParse().loadAddress();
  }

  readSlice(): c.Slice {
    return this.popExpecting<c.TupleItemSlice>('slice').cell.beginParse();
  }

  readBigIntOpt(): bigint | null {
    const item = this.tuple.shift();
    if (item?.type === 'null') {
      return null;
    }
    if (item?.type !== 'int') {
      throw new Error(`not 'int' on a stack`);
    }
    return item.value;
  }

  readWideNullable<T>(
    stackW: number,
    readFn_T: (r: StackReader) => T,
  ): T | null {
    const slotTypeId = this.tuple[stackW - 1];
    if (slotTypeId?.type !== 'int') {
      throw new Error(`not 'int' on a stack`);
    }
    if (slotTypeId.value === 0n) {
      this.tuple = this.tuple.slice(stackW);
      return null;
    }
    const valueT = readFn_T(this);
    this.tuple.shift();
    return valueT;
  }

  readNullable<T>(readFn_T: (r: StackReader) => T): T | null {
    return this.readWideNullable(2, readFn_T);
  }
}

// ————————————————————————————————————————————
//   auto-generated serializers to/from cells
//

type coins = bigint;

type int8 = bigint;
type int16 = bigint;
type int32 = bigint;
type int256 = bigint;

type uint8 = bigint;
type uint16 = bigint;
type uint32 = bigint;
type uint64 = bigint;
type uint256 = bigint;

/**
 > struct (0x43c7641f) TopUp {
 > }
 */
export interface TopUp {
  readonly $: 'TopUp';
}

export const TopUp = {
  PREFIX: 0x43c7641f,

  create(): TopUp {
    return {
      $: 'TopUp',
    };
  },
  fromSlice(s: c.Slice): TopUp {
    loadAndCheckPrefix32(s, 0x43c7641f, 'TopUp');
    return {
      $: 'TopUp',
    };
  },
  store(self: TopUp, b: c.Builder): void {
    b.storeUint(0x43c7641f, 32);
  },
  toCell(self: TopUp): c.Cell {
    return makeCellFrom<TopUp>(self, TopUp.store);
  },
};

/**
 > struct (0x283b4c3f) CancelSubscription {
 >     queryId: uint64
 > }
 */
export interface CancelSubscription {
  readonly $: 'CancelSubscription';
  queryId: uint64;
}

export const CancelSubscription = {
  PREFIX: 0x283b4c3f,

  create(args: { queryId: uint64 }): CancelSubscription {
    return {
      $: 'CancelSubscription',
      ...args,
    };
  },
  fromSlice(s: c.Slice): CancelSubscription {
    loadAndCheckPrefix32(s, 0x283b4c3f, 'CancelSubscription');
    return {
      $: 'CancelSubscription',
      queryId: s.loadUintBig(64),
    };
  },
  store(self: CancelSubscription, b: c.Builder): void {
    b.storeUint(0x283b4c3f, 32);
    b.storeUint(self.queryId, 64);
  },
  toCell(self: CancelSubscription): c.Cell {
    return makeCellFrom<CancelSubscription>(self, CancelSubscription.store);
  },
};

/**
 > struct (0xc28364ef) ReceivePaymentFromWallet {
 > }
 */
export interface ReceivePaymentFromWallet {
  readonly $: 'ReceivePaymentFromWallet';
}

export const ReceivePaymentFromWallet = {
  PREFIX: 0xc28364ef,

  create(): ReceivePaymentFromWallet {
    return {
      $: 'ReceivePaymentFromWallet',
    };
  },
  fromSlice(s: c.Slice): ReceivePaymentFromWallet {
    loadAndCheckPrefix32(s, 0xc28364ef, 'ReceivePaymentFromWallet');
    return {
      $: 'ReceivePaymentFromWallet',
    };
  },
  store(self: ReceivePaymentFromWallet, b: c.Builder): void {
    b.storeUint(0xc28364ef, 32);
  },
  toCell(self: ReceivePaymentFromWallet): c.Cell {
    return makeCellFrom<ReceivePaymentFromWallet>(
      self,
      ReceivePaymentFromWallet.store,
    );
  },
};

/**
 > struct (0x7e8764ef) CollectPaymentFromWallet {
 >     queryId: uint64
 > }
 */
export interface CollectPaymentFromWallet {
  readonly $: 'CollectPaymentFromWallet';
  queryId: uint64;
}

export const CollectPaymentFromWallet = {
  PREFIX: 0x7e8764ef,

  create(args: { queryId: uint64 }): CollectPaymentFromWallet {
    return {
      $: 'CollectPaymentFromWallet',
      ...args,
    };
  },
  fromSlice(s: c.Slice): CollectPaymentFromWallet {
    loadAndCheckPrefix32(s, 0x7e8764ef, 'CollectPaymentFromWallet');
    return {
      $: 'CollectPaymentFromWallet',
      queryId: s.loadUintBig(64),
    };
  },
  store(self: CollectPaymentFromWallet, b: c.Builder): void {
    b.storeUint(0x7e8764ef, 32);
    b.storeUint(self.queryId, 64);
  },
  toCell(self: CollectPaymentFromWallet): c.Cell {
    return makeCellFrom<CollectPaymentFromWallet>(
      self,
      CollectPaymentFromWallet.store,
    );
  },
};

/**
 > struct (0x342364ac) WithdrawExtensionBalance {
 > }
 */
export interface WithdrawExtensionBalance {
  readonly $: 'WithdrawExtensionBalance';
}

export const WithdrawExtensionBalance = {
  PREFIX: 0x342364ac,

  create(): WithdrawExtensionBalance {
    return {
      $: 'WithdrawExtensionBalance',
    };
  },
  fromSlice(s: c.Slice): WithdrawExtensionBalance {
    loadAndCheckPrefix32(s, 0x342364ac, 'WithdrawExtensionBalance');
    return {
      $: 'WithdrawExtensionBalance',
    };
  },
  store(self: WithdrawExtensionBalance, b: c.Builder): void {
    b.storeUint(0x342364ac, 32);
  },
  toCell(self: WithdrawExtensionBalance): c.Cell {
    return makeCellFrom<WithdrawExtensionBalance>(
      self,
      WithdrawExtensionBalance.store,
    );
  },
};

/**
 > struct ExtensionStorage {
 >     walletAddress: address
 >     admin: address
 >     subscriptionAmount: coins
 >     lastPaymentTime: uint32?
 >     paymentTimeInterval: uint32
 > }
 */
export interface ExtensionStorage {
  readonly $: 'ExtensionStorage';
  walletAddress: c.Address;
  admin: c.Address;
  subscriptionAmount: coins;
  lastPaymentTime: uint32 | null;
  paymentTimeInterval: uint32;
}

export const ExtensionStorage = {
  create(args: {
    walletAddress: c.Address;
    admin: c.Address;
    subscriptionAmount: coins;
    lastPaymentTime: uint32 | null;
    paymentTimeInterval: uint32;
  }): ExtensionStorage {
    return {
      $: 'ExtensionStorage',
      ...args,
    };
  },
  fromSlice(s: c.Slice): ExtensionStorage {
    return {
      $: 'ExtensionStorage',
      walletAddress: s.loadAddress(),
      admin: s.loadAddress(),
      subscriptionAmount: s.loadCoins(),
      lastPaymentTime: s.loadBoolean() ? s.loadUintBig(32) : null,
      paymentTimeInterval: s.loadUintBig(32),
    };
  },
  store(self: ExtensionStorage, b: c.Builder): void {
    b.storeAddress(self.walletAddress);
    b.storeAddress(self.admin);
    b.storeCoins(self.subscriptionAmount);
    storeTolkNullable<uint32>(self.lastPaymentTime, b, (v, b) =>
      b.storeUint(v, 32),
    );
    b.storeUint(self.paymentTimeInterval, 32);
  },
  toCell(self: ExtensionStorage): c.Cell {
    return makeCellFrom<ExtensionStorage>(self, ExtensionStorage.store);
  },
};

/**
 > struct (0x6578746e) W5ExtensionActionRequest {
 >     queryId: uint64
 >     outActions: cell?
 >     hasExtraActions: bool
 >     extraActions: RemainingBitsAndRefs
 > }
 */
export interface W5ExtensionActionRequest {
  readonly $: 'W5ExtensionActionRequest';
  queryId: uint64;
  outActions: c.Cell | null;
  hasExtraActions: boolean;
  extraActions: RemainingBitsAndRefs;
}

export const W5ExtensionActionRequest = {
  PREFIX: 0x6578746e,

  create(args: {
    queryId: uint64;
    outActions: c.Cell | null;
    hasExtraActions: boolean;
    extraActions: RemainingBitsAndRefs;
  }): W5ExtensionActionRequest {
    return {
      $: 'W5ExtensionActionRequest',
      ...args,
    };
  },
  fromSlice(s: c.Slice): W5ExtensionActionRequest {
    loadAndCheckPrefix32(s, 0x6578746e, 'W5ExtensionActionRequest');
    return {
      $: 'W5ExtensionActionRequest',
      queryId: s.loadUintBig(64),
      outActions: s.loadBoolean() ? s.loadRef() : null,
      hasExtraActions: s.loadBoolean(),
      extraActions: loadTolkRemaining(s),
    };
  },
  store(self: W5ExtensionActionRequest, b: c.Builder): void {
    b.storeUint(0x6578746e, 32);
    b.storeUint(self.queryId, 64);
    storeTolkNullable<c.Cell>(self.outActions, b, (v, b) => b.storeRef(v));
    b.storeBit(self.hasExtraActions);
    storeTolkRemaining(self.extraActions, b);
  },
  toCell(self: W5ExtensionActionRequest): c.Cell {
    return makeCellFrom<W5ExtensionActionRequest>(
      self,
      W5ExtensionActionRequest.store,
    );
  },
};

// ————————————————————————————————————————————
//    class SimpleExtension
//

interface ExtraSendOptions {
  bounce?: boolean; // default: false
  sendMode?: SendMode; // default: SendMode.PAY_GAS_SEPARATELY
  extraCurrencies?: c.ExtraCurrency; // default: empty dict
}

interface DeployedAddrOptions {
  workchain?: number; // default: 0 (basechain)
  toShard?: { fixedPrefixLength: number; closeTo: c.Address };
  overrideContractCode?: c.Cell;
}

function calculateDeployedAddress(
  code: c.Cell,
  data: c.Cell,
  options: DeployedAddrOptions,
): c.Address {
  const stateInitCell = beginCell()
    .store(
      c.storeStateInit({
        code,
        data,
        splitDepth: options.toShard?.fixedPrefixLength,
        special: null, // todo will somebody need special?
        libraries: null, // todo will somebody need libraries?
      }),
    )
    .endCell();

  let addrHash = stateInitCell.hash();
  if (options.toShard) {
    const shardDepth = options.toShard.fixedPrefixLength;
    addrHash = beginCell() // todo any way to do it better? N bits from closeTo + 256-N from stateInitCell
      .storeBits(new c.BitString(options.toShard.closeTo.hash, 0, shardDepth))
      .storeBits(
        new c.BitString(stateInitCell.hash(), shardDepth, 256 - shardDepth),
      )
      .endCell()
      .beginParse()
      .loadBuffer(32);
  }

  return new c.Address(options.workchain ?? 0, addrHash);
}

export class SimpleExtension implements c.Contract {
  static CodeCell = c.Cell.fromBase64(
    'te6ccgECCQEAAaMAART/APSkE/S88sgLAQIBIAIDAgFIBAUBpvLXLCP0Oyd84wLXLCGhGyVkMY467UTQ+kgx+kj6ADD4J28QAYIQHc1lAKC88uP0+AD4J28QghAdzWUAocjPhQgS+lIB+gJwzwtqyXD7AOCED/LwBwHA0PiR8kDXLCFB2mH8jj/tRND6SPpIMPiSxwXy4/UB1ws/bfgoyM+EDvpSz1DIz5GV4dG6E8s/9ADPg87JyM+FCBL6UnHPC27MyYBA+wDg1ywmFBsnfOMC1ywiHjsg/DHcBgAxoZoV2omh9JH0kfQBpgADJaY/JNoDxaY/owBwMO1E0PpI+kj6ANMAAZPTHzHe+JIkxwXy4/b4IwTI+lIT+lIB+gIibpRsEs+Blc+DEssf4s7J7VQB8u1E0PpI+kgx+gDTAAGS0x+SbQHi1wsfIW6SW3+WoPgjucMA4vLj8/gA+CjIz4UI+lIB+gKCEMKDZO/PC4rJiMjMz5Q7DyG0DszJAtcLP4IK+vCAiwjIz5GV4dG6E8s/FPQAz4HOycjPhQgS+lJY+gJxzwtqzMlw+wAIAAA=',
  );

  static Errors = {
    'Errors.PaymentIntervalNotPassed': 1011,
    'Errors.NotEnoughExtensionBalance': 1012,
    'Errors.OnlyAdmin': 1013,
    'Errors.OnlyWallet': 1014,
    'Errors.UnsupportedExternalMessage': 65535,
  };

  readonly address: c.Address;
  readonly init?: { code: c.Cell; data: c.Cell };

  private constructor(
    address: c.Address,
    init?: { code: c.Cell; data: c.Cell },
  ) {
    this.address = address;
    this.init = init;
  }

  static fromAddress(address: c.Address) {
    return new SimpleExtension(address);
  }

  static fromStorage(
    emptyStorage: {
      walletAddress: c.Address;
      admin: c.Address;
      subscriptionAmount: coins;
      lastPaymentTime: uint32 | null;
      paymentTimeInterval: uint32;
    },
    deployedOptions?: DeployedAddrOptions,
  ) {
    const initialState = {
      code: deployedOptions?.overrideContractCode ?? SimpleExtension.CodeCell,
      data: ExtensionStorage.toCell(ExtensionStorage.create(emptyStorage)),
    };
    const address = calculateDeployedAddress(
      initialState.code,
      initialState.data,
      deployedOptions ?? {},
    );
    return new SimpleExtension(address, initialState);
  }

  static createCellOfTopUp(body: {}) {
    return TopUp.toCell(TopUp.create());
  }

  static createCellOfCancelSubscription(body: { queryId: uint64 }) {
    return CancelSubscription.toCell(CancelSubscription.create(body));
  }

  static createCellOfReceivePaymentFromWallet(body: {}) {
    return ReceivePaymentFromWallet.toCell(ReceivePaymentFromWallet.create());
  }

  async sendDeploy(
    provider: ContractProvider,
    via: Sender,
    msgValue: coins,
    extraOptions?: ExtraSendOptions,
  ) {
    return provider.internal(via, {
      value: msgValue,
      body: c.Cell.EMPTY,
      ...extraOptions,
    });
  }

  async sendTopUp(
    provider: ContractProvider,
    via: Sender,
    msgValue: coins,
    body: {},
    extraOptions?: ExtraSendOptions,
  ) {
    return provider.internal(via, {
      value: msgValue,
      body: TopUp.toCell(TopUp.create()),
      ...extraOptions,
    });
  }

  async sendCancelSubscription(
    provider: ContractProvider,
    via: Sender,
    msgValue: coins,
    body: {
      queryId: uint64;
    },
    extraOptions?: ExtraSendOptions,
  ) {
    return provider.internal(via, {
      value: msgValue,
      body: CancelSubscription.toCell(CancelSubscription.create(body)),
      ...extraOptions,
    });
  }

  async sendReceivePaymentFromWallet(
    provider: ContractProvider,
    via: Sender,
    msgValue: coins,
    body: {},
    extraOptions?: ExtraSendOptions,
  ) {
    return provider.internal(via, {
      value: msgValue,
      body: ReceivePaymentFromWallet.toCell(ReceivePaymentFromWallet.create()),
      ...extraOptions,
    });
  }

  async getExtensionInfo(
    provider: ContractProvider,
  ): Promise<ExtensionStorage> {
    const r = StackReader.fromGetMethod(
      5,
      await provider.get('extensionInfo', []),
    );
    return {
      $: 'ExtensionStorage',
      walletAddress: r.readAddress(),
      admin: r.readAddress(),
      subscriptionAmount: r.readBigInt(),
      lastPaymentTime: r.readBigIntOpt(),
      paymentTimeInterval: r.readBigInt(),
    };
  }
}
