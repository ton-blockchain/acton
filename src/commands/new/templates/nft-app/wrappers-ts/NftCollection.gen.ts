// AUTO-GENERATED, do not edit
// it's a TypeScript wrapper for a NftCollection contract in Tolk
/* eslint-disable */

import * as c from '@ton/core';
import { beginCell, ContractProvider, Sender, SendMode } from '@ton/core';

// ————————————————————————————————————————————
//   predefined types and functions
//

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

function formatPrefix(prefixNum: number, prefixLen: number): string {
  return prefixLen % 4
    ? `0b${prefixNum.toString(2).padStart(prefixLen, '0')}`
    : `0x${prefixNum.toString(16).padStart(prefixLen / 4, '0')}`;
}

function loadAndCheckPrefix(
  s: c.Slice,
  expected: number,
  prefixLen: number,
  structName: string,
): void {
  let prefix = s.loadUint(prefixLen);
  if (prefix !== expected) {
    throw new Error(
      `Incorrect prefix for '${structName}': expected ${formatPrefix(expected, prefixLen)}, got ${formatPrefix(prefix, prefixLen)}`,
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

function createDictionaryValue<V>(
  loadFn_V: LoadCallback<V>,
  storeFn_V: StoreCallback<V>,
): c.DictionaryValue<V> {
  return {
    serialize(self: V, b: c.Builder) {
      storeFn_V(self, b);
    },
    parse(s: c.Slice): V {
      const value = loadFn_V(s);
      s.endParse();
      return value;
    },
  };
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
    return this.popExpecting<c.TupleItemCell>('cell').cell;
  }

  readSlice(): c.Slice {
    return this.popExpecting<c.TupleItemSlice>('slice').cell.beginParse();
  }

  readCellRef<T>(loadFn_T: LoadCallback<T>): CellRef<T> {
    return { ref: loadFn_T(this.readCell().beginParse()) };
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
 > struct (0x693d3950) RequestRoyaltyParams {
 >     queryId: uint64
 > }
 */
export interface RequestRoyaltyParams {
  readonly $: 'RequestRoyaltyParams';
  queryId: uint64;
}

export const RequestRoyaltyParams = {
  PREFIX: 0x693d3950,

  create(args: { queryId: uint64 }): RequestRoyaltyParams {
    return {
      $: 'RequestRoyaltyParams',
      ...args,
    };
  },
  fromSlice(s: c.Slice): RequestRoyaltyParams {
    loadAndCheckPrefix32(s, 0x693d3950, 'RequestRoyaltyParams');
    return {
      $: 'RequestRoyaltyParams',
      queryId: s.loadUintBig(64),
    };
  },
  store(self: RequestRoyaltyParams, b: c.Builder): void {
    b.storeUint(0x693d3950, 32);
    b.storeUint(self.queryId, 64);
  },
  toCell(self: RequestRoyaltyParams): c.Cell {
    return makeCellFrom<RequestRoyaltyParams>(self, RequestRoyaltyParams.store);
  },
};

/**
 > struct NftItemInitAtDeployment {
 >     ownerAddress: address
 >     content: string
 > }
 */
export interface NftItemInitAtDeployment {
  readonly $: 'NftItemInitAtDeployment';
  ownerAddress: c.Address;
  content: string;
}

export const NftItemInitAtDeployment = {
  create(args: {
    ownerAddress: c.Address;
    content: string;
  }): NftItemInitAtDeployment {
    return {
      $: 'NftItemInitAtDeployment',
      ...args,
    };
  },
  fromSlice(s: c.Slice): NftItemInitAtDeployment {
    return {
      $: 'NftItemInitAtDeployment',
      ownerAddress: s.loadAddress(),
      content: s.loadStringRefTail(),
    };
  },
  store(self: NftItemInitAtDeployment, b: c.Builder): void {
    b.storeAddress(self.ownerAddress);
    b.storeStringRefTail(self.content);
  },
  toCell(self: NftItemInitAtDeployment): c.Cell {
    return makeCellFrom<NftItemInitAtDeployment>(
      self,
      NftItemInitAtDeployment.store,
    );
  },
};

/**
 > struct (0x00000001) DeployNft {
 >     queryId: uint64
 >     itemIndex: uint64
 >     attachTonAmount: coins
 >     initParams: Cell<NftItemInitAtDeployment>
 > }
 */
export interface DeployNft {
  readonly $: 'DeployNft';
  queryId: uint64;
  itemIndex: uint64;
  attachTonAmount: coins;
  initParams: CellRef<NftItemInitAtDeployment>;
}

export const DeployNft = {
  PREFIX: 0x00000001,

  create(args: {
    queryId: uint64;
    itemIndex: uint64;
    attachTonAmount: coins;
    initParams: CellRef<NftItemInitAtDeployment>;
  }): DeployNft {
    return {
      $: 'DeployNft',
      ...args,
    };
  },
  fromSlice(s: c.Slice): DeployNft {
    loadAndCheckPrefix32(s, 0x00000001, 'DeployNft');
    return {
      $: 'DeployNft',
      queryId: s.loadUintBig(64),
      itemIndex: s.loadUintBig(64),
      attachTonAmount: s.loadCoins(),
      initParams: loadCellRef<NftItemInitAtDeployment>(
        s,
        NftItemInitAtDeployment.fromSlice,
      ),
    };
  },
  store(self: DeployNft, b: c.Builder): void {
    b.storeUint(0x00000001, 32);
    b.storeUint(self.queryId, 64);
    b.storeUint(self.itemIndex, 64);
    b.storeCoins(self.attachTonAmount);
    storeCellRef<NftItemInitAtDeployment>(
      self.initParams,
      b,
      NftItemInitAtDeployment.store,
    );
  },
  toCell(self: DeployNft): c.Cell {
    return makeCellFrom<DeployNft>(self, DeployNft.store);
  },
};

/**
 > struct BatchDeployDictItem {
 >     attachTonAmount: coins
 >     initParams: Cell<NftItemInitAtDeployment>
 > }
 */
export interface BatchDeployDictItem {
  readonly $: 'BatchDeployDictItem';
  attachTonAmount: coins;
  initParams: CellRef<NftItemInitAtDeployment>;
}

export const BatchDeployDictItem = {
  create(args: {
    attachTonAmount: coins;
    initParams: CellRef<NftItemInitAtDeployment>;
  }): BatchDeployDictItem {
    return {
      $: 'BatchDeployDictItem',
      ...args,
    };
  },
  fromSlice(s: c.Slice): BatchDeployDictItem {
    return {
      $: 'BatchDeployDictItem',
      attachTonAmount: s.loadCoins(),
      initParams: loadCellRef<NftItemInitAtDeployment>(
        s,
        NftItemInitAtDeployment.fromSlice,
      ),
    };
  },
  store(self: BatchDeployDictItem, b: c.Builder): void {
    b.storeCoins(self.attachTonAmount);
    storeCellRef<NftItemInitAtDeployment>(
      self.initParams,
      b,
      NftItemInitAtDeployment.store,
    );
  },
  toCell(self: BatchDeployDictItem): c.Cell {
    return makeCellFrom<BatchDeployDictItem>(self, BatchDeployDictItem.store);
  },
};

/**
 > struct (0x00000002) BatchDeployNfts {
 >     queryId: uint64
 >     deployList: map<uint64, BatchDeployDictItem>
 > }
 */
export interface BatchDeployNfts {
  readonly $: 'BatchDeployNfts';
  queryId: uint64;
  deployList: c.Dictionary<uint64, BatchDeployDictItem>;
}

export const BatchDeployNfts = {
  PREFIX: 0x00000002,

  create(args: {
    queryId: uint64;
    deployList: c.Dictionary<uint64, BatchDeployDictItem>;
  }): BatchDeployNfts {
    return {
      $: 'BatchDeployNfts',
      ...args,
    };
  },
  fromSlice(s: c.Slice): BatchDeployNfts {
    loadAndCheckPrefix32(s, 0x00000002, 'BatchDeployNfts');
    return {
      $: 'BatchDeployNfts',
      queryId: s.loadUintBig(64),
      deployList: c.Dictionary.load<uint64, BatchDeployDictItem>(
        c.Dictionary.Keys.BigUint(64),
        createDictionaryValue<BatchDeployDictItem>(
          BatchDeployDictItem.fromSlice,
          BatchDeployDictItem.store,
        ),
        s,
      ),
    };
  },
  store(self: BatchDeployNfts, b: c.Builder): void {
    b.storeUint(0x00000002, 32);
    b.storeUint(self.queryId, 64);
    b.storeDict<uint64, BatchDeployDictItem>(
      self.deployList,
      c.Dictionary.Keys.BigUint(64),
      createDictionaryValue<BatchDeployDictItem>(
        BatchDeployDictItem.fromSlice,
        BatchDeployDictItem.store,
      ),
    );
  },
  toCell(self: BatchDeployNfts): c.Cell {
    return makeCellFrom<BatchDeployNfts>(self, BatchDeployNfts.store);
  },
};

/**
 > struct (0x00000003) ChangeCollectionAdmin {
 >     queryId: uint64
 >     newAdminAddress: address
 > }
 */
export interface ChangeCollectionAdmin {
  readonly $: 'ChangeCollectionAdmin';
  queryId: uint64;
  newAdminAddress: c.Address;
}

export const ChangeCollectionAdmin = {
  PREFIX: 0x00000003,

  create(args: {
    queryId: uint64;
    newAdminAddress: c.Address;
  }): ChangeCollectionAdmin {
    return {
      $: 'ChangeCollectionAdmin',
      ...args,
    };
  },
  fromSlice(s: c.Slice): ChangeCollectionAdmin {
    loadAndCheckPrefix32(s, 0x00000003, 'ChangeCollectionAdmin');
    return {
      $: 'ChangeCollectionAdmin',
      queryId: s.loadUintBig(64),
      newAdminAddress: s.loadAddress(),
    };
  },
  store(self: ChangeCollectionAdmin, b: c.Builder): void {
    b.storeUint(0x00000003, 32);
    b.storeUint(self.queryId, 64);
    b.storeAddress(self.newAdminAddress);
  },
  toCell(self: ChangeCollectionAdmin): c.Cell {
    return makeCellFrom<ChangeCollectionAdmin>(
      self,
      ChangeCollectionAdmin.store,
    );
  },
};

/**
 > struct CollectionContent {
 >     collectionMetadata: cell
 >     commonContent: string
 > }
 */
export interface CollectionContent {
  readonly $: 'CollectionContent';
  collectionMetadata: c.Cell;
  commonContent: string;
}

export const CollectionContent = {
  create(args: {
    collectionMetadata: c.Cell;
    commonContent: string;
  }): CollectionContent {
    return {
      $: 'CollectionContent',
      ...args,
    };
  },
  fromSlice(s: c.Slice): CollectionContent {
    return {
      $: 'CollectionContent',
      collectionMetadata: s.loadRef(),
      commonContent: s.loadStringRefTail(),
    };
  },
  store(self: CollectionContent, b: c.Builder): void {
    b.storeRef(self.collectionMetadata);
    b.storeStringRefTail(self.commonContent);
  },
  toCell(self: CollectionContent): c.Cell {
    return makeCellFrom<CollectionContent>(self, CollectionContent.store);
  },
};

/**
 > struct RoyaltyParams {
 >     numerator: uint16
 >     denominator: uint16
 >     royaltyAddress: address
 > }
 */
export interface RoyaltyParams {
  readonly $: 'RoyaltyParams';
  numerator: uint16;
  denominator: uint16;
  royaltyAddress: c.Address;
}

export const RoyaltyParams = {
  create(args: {
    numerator: uint16;
    denominator: uint16;
    royaltyAddress: c.Address;
  }): RoyaltyParams {
    return {
      $: 'RoyaltyParams',
      ...args,
    };
  },
  fromSlice(s: c.Slice): RoyaltyParams {
    return {
      $: 'RoyaltyParams',
      numerator: s.loadUintBig(16),
      denominator: s.loadUintBig(16),
      royaltyAddress: s.loadAddress(),
    };
  },
  store(self: RoyaltyParams, b: c.Builder): void {
    b.storeUint(self.numerator, 16);
    b.storeUint(self.denominator, 16);
    b.storeAddress(self.royaltyAddress);
  },
  toCell(self: RoyaltyParams): c.Cell {
    return makeCellFrom<RoyaltyParams>(self, RoyaltyParams.store);
  },
};

/**
 > struct NftCollectionStorage {
 >     adminAddress: address
 >     nextItemIndex: uint64
 >     content: Cell<CollectionContent>
 >     nftItemCode: cell
 >     royaltyParams: Cell<RoyaltyParams>
 > }
 */
export interface NftCollectionStorage {
  readonly $: 'NftCollectionStorage';
  adminAddress: c.Address;
  nextItemIndex: uint64;
  content: CellRef<CollectionContent>;
  nftItemCode: c.Cell;
  royaltyParams: CellRef<RoyaltyParams>;
}

export const NftCollectionStorage = {
  create(args: {
    adminAddress: c.Address;
    nextItemIndex: uint64;
    content: CellRef<CollectionContent>;
    nftItemCode: c.Cell;
    royaltyParams: CellRef<RoyaltyParams>;
  }): NftCollectionStorage {
    return {
      $: 'NftCollectionStorage',
      ...args,
    };
  },
  fromSlice(s: c.Slice): NftCollectionStorage {
    return {
      $: 'NftCollectionStorage',
      adminAddress: s.loadAddress(),
      nextItemIndex: s.loadUintBig(64),
      content: loadCellRef<CollectionContent>(s, CollectionContent.fromSlice),
      nftItemCode: s.loadRef(),
      royaltyParams: loadCellRef<RoyaltyParams>(s, RoyaltyParams.fromSlice),
    };
  },
  store(self: NftCollectionStorage, b: c.Builder): void {
    b.storeAddress(self.adminAddress);
    b.storeUint(self.nextItemIndex, 64);
    storeCellRef<CollectionContent>(self.content, b, CollectionContent.store);
    b.storeRef(self.nftItemCode);
    storeCellRef<RoyaltyParams>(self.royaltyParams, b, RoyaltyParams.store);
  },
  toCell(self: NftCollectionStorage): c.Cell {
    return makeCellFrom<NftCollectionStorage>(self, NftCollectionStorage.store);
  },
};

/**
 > struct (0xa8cb00ad) ResponseRoyaltyParams {
 >     queryId: uint64
 >     royaltyParams: RoyaltyParams
 > }
 */
export interface ResponseRoyaltyParams {
  readonly $: 'ResponseRoyaltyParams';
  queryId: uint64;
  royaltyParams: RoyaltyParams;
}

export const ResponseRoyaltyParams = {
  PREFIX: 0xa8cb00ad,

  create(args: {
    queryId: uint64;
    royaltyParams: RoyaltyParams;
  }): ResponseRoyaltyParams {
    return {
      $: 'ResponseRoyaltyParams',
      ...args,
    };
  },
  fromSlice(s: c.Slice): ResponseRoyaltyParams {
    loadAndCheckPrefix32(s, 0xa8cb00ad, 'ResponseRoyaltyParams');
    return {
      $: 'ResponseRoyaltyParams',
      queryId: s.loadUintBig(64),
      royaltyParams: RoyaltyParams.fromSlice(s),
    };
  },
  store(self: ResponseRoyaltyParams, b: c.Builder): void {
    b.storeUint(0xa8cb00ad, 32);
    b.storeUint(self.queryId, 64);
    RoyaltyParams.store(self.royaltyParams, b);
  },
  toCell(self: ResponseRoyaltyParams): c.Cell {
    return makeCellFrom<ResponseRoyaltyParams>(
      self,
      ResponseRoyaltyParams.store,
    );
  },
};

/**
 > struct CollectionDataReply {
 >     nextItemIndex: int
 >     collectionMetadata: cell
 >     adminAddress: address
 > }
 */
export interface CollectionDataReply {
  readonly $: 'CollectionDataReply';
  nextItemIndex: bigint;
  collectionMetadata: c.Cell;
  adminAddress: c.Address;
}

export const CollectionDataReply = {
  create(args: {
    nextItemIndex: bigint;
    collectionMetadata: c.Cell;
    adminAddress: c.Address;
  }): CollectionDataReply {
    return {
      $: 'CollectionDataReply',
      ...args,
    };
  },
  fromSlice(s: c.Slice): CollectionDataReply {
    throw new Error(
      `Can't unpack 'CollectionDataReply' from cell, because 'CollectionDataReply.nextItemIndex' is 'int' (not int32/uint64/etc.)`,
    );
  },
  store(self: CollectionDataReply, b: c.Builder): void {
    throw new Error(
      `Can't pack 'CollectionDataReply' to cell, because 'self.nextItemIndex' is 'int' (not int32/uint64/etc.)`,
    );
  },
  toCell(self: CollectionDataReply): c.Cell {
    return makeCellFrom<CollectionDataReply>(self, CollectionDataReply.store);
  },
};

/**
 > struct (0x01) OffchainMetadataReply {
 >     string: string
 > }
 */
export interface OffchainMetadataReply {
  readonly $: 'OffchainMetadataReply';
  string: string;
}

export const OffchainMetadataReply = {
  PREFIX: 0x01,

  create(args: { string: string }): OffchainMetadataReply {
    return {
      $: 'OffchainMetadataReply',
      ...args,
    };
  },
  fromSlice(s: c.Slice): OffchainMetadataReply {
    loadAndCheckPrefix(s, 0x01, 8, 'OffchainMetadataReply');
    return {
      $: 'OffchainMetadataReply',
      string: s.loadStringRefTail(),
    };
  },
  store(self: OffchainMetadataReply, b: c.Builder): void {
    b.storeUint(0x01, 8);
    b.storeStringRefTail(self.string);
  },
  toCell(self: OffchainMetadataReply): c.Cell {
    return makeCellFrom<OffchainMetadataReply>(
      self,
      OffchainMetadataReply.store,
    );
  },
};

// ————————————————————————————————————————————
//    class NftCollection
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

export class NftCollection implements c.Contract {
  static CodeCell = c.Cell.fromBase64(
    'te6ccgECEQEAAnAAART/APSkE/S88sgLAQIBYgIDAvjQ+JGRMOAg1ywgAAAADI5oMe1E0PpI0z8g1DHXTPiSJMcF8uGRBNM/MdM/+gDXTFMku/LhklMkuvgoBMjLPxT6UsnIz4mIAVMYyM+E0MzM+RbPC/9QA/oCgQCNzwtwF8zMFczJcfsAA5ukAcj6Uss/zsntVJJfA+LgidcnBAUCASAKCwAIaT05UAT+jjkx7UTQAdcLPwHUMdQx10z4kgHQ0w/TD/pI0cjPhQgU+lKCEKjLAK3PC44Uyz/LDxLLD/pSyYBA+wDg1ywgAAAAFI6zMe1E0PpI0z8g1DHXTPiSJMcF8uGRcAXTPzH0BSCAQPSGb6WQiuhfBDMByPpSyz/Oye1U4InXJ+MCMAYHCAkAvAekIIEA+rny4Y9Ud3W7mhdfB4EBkTKg8vDhAvoA1NH4KCTIyz/6UskmyM+JiAFTIcjPhNDMzPkWzwv/UAT6AoEAjc8LcBPMEszMyXH7AFEVupMEpATeUWGAQPR8b6UACAAAAAMAODHtRND6SPiSWMcF8uGRAdM/MfpIMMj6Us7J7VQADoQPAccA8vQCASAMDQAfvILfaiaH0kaZ/rpmhrpixAE7uLXTHtRNDXTNDUMddMbwABb4wBb4zbPMjPhAbMyYDgIBIA8QAIggb4shb4ilII43pVMgb4HQINdkjiVvACHXZJcB1AHQWW+M5AFvjCBviKUgmlxvgcjOFMzJA6XkMG8Q3sjOEszJAeQwMQAntdr9qJoahjqGOumaGmH6Yf9JGjAAS7T0faiaGoY66Z8FAFkZZ+JfSlkgORnwmhmZnyLZGfFACBl/+eoQ',
  );

  static Errors = {
    ERROR_BATCH_LIMIT_EXCEEDED: 399,
    ERROR_NOT_FROM_ADMIN: 401,
    ERROR_INVALID_ITEM_INDEX: 402,
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
    return new NftCollection(address);
  }

  static fromStorage(
    emptyStorage: {
      adminAddress: c.Address;
      nextItemIndex: uint64;
      content: CellRef<CollectionContent>;
      nftItemCode: c.Cell;
      royaltyParams: CellRef<RoyaltyParams>;
    },
    deployedOptions?: DeployedAddrOptions,
  ) {
    const initialState = {
      code: deployedOptions?.overrideContractCode ?? NftCollection.CodeCell,
      data: NftCollectionStorage.toCell(
        NftCollectionStorage.create(emptyStorage),
      ),
    };
    const address = calculateDeployedAddress(
      initialState.code,
      initialState.data,
      deployedOptions ?? {},
    );
    return new NftCollection(address, initialState);
  }

  static createCellOfRequestRoyaltyParams(body: { queryId: uint64 }) {
    return RequestRoyaltyParams.toCell(RequestRoyaltyParams.create(body));
  }

  static createCellOfDeployNft(body: {
    queryId: uint64;
    itemIndex: uint64;
    attachTonAmount: coins;
    initParams: CellRef<NftItemInitAtDeployment>;
  }) {
    return DeployNft.toCell(DeployNft.create(body));
  }

  static createCellOfBatchDeployNfts(body: {
    queryId: uint64;
    deployList: c.Dictionary<uint64, BatchDeployDictItem>;
  }) {
    return BatchDeployNfts.toCell(BatchDeployNfts.create(body));
  }

  static createCellOfChangeCollectionAdmin(body: {
    queryId: uint64;
    newAdminAddress: c.Address;
  }) {
    return ChangeCollectionAdmin.toCell(ChangeCollectionAdmin.create(body));
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

  async sendRequestRoyaltyParams(
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
      body: RequestRoyaltyParams.toCell(RequestRoyaltyParams.create(body)),
      ...extraOptions,
    });
  }

  async sendDeployNft(
    provider: ContractProvider,
    via: Sender,
    msgValue: coins,
    body: {
      queryId: uint64;
      itemIndex: uint64;
      attachTonAmount: coins;
      initParams: CellRef<NftItemInitAtDeployment>;
    },
    extraOptions?: ExtraSendOptions,
  ) {
    return provider.internal(via, {
      value: msgValue,
      body: DeployNft.toCell(DeployNft.create(body)),
      ...extraOptions,
    });
  }

  async sendBatchDeployNfts(
    provider: ContractProvider,
    via: Sender,
    msgValue: coins,
    body: {
      queryId: uint64;
      deployList: c.Dictionary<uint64, BatchDeployDictItem>;
    },
    extraOptions?: ExtraSendOptions,
  ) {
    return provider.internal(via, {
      value: msgValue,
      body: BatchDeployNfts.toCell(BatchDeployNfts.create(body)),
      ...extraOptions,
    });
  }

  async sendChangeCollectionAdmin(
    provider: ContractProvider,
    via: Sender,
    msgValue: coins,
    body: {
      queryId: uint64;
      newAdminAddress: c.Address;
    },
    extraOptions?: ExtraSendOptions,
  ) {
    return provider.internal(via, {
      value: msgValue,
      body: ChangeCollectionAdmin.toCell(ChangeCollectionAdmin.create(body)),
      ...extraOptions,
    });
  }

  async getCollectionData(
    provider: ContractProvider,
  ): Promise<CollectionDataReply> {
    const r = StackReader.fromGetMethod(
      3,
      await provider.get('get_collection_data', []),
    );
    return {
      $: 'CollectionDataReply',
      nextItemIndex: r.readBigInt(),
      collectionMetadata: r.readCell(),
      adminAddress: r.readSlice().loadAddress(),
    };
  }

  async getNftAddressByIndex(
    provider: ContractProvider,
    itemIndex: bigint,
  ): Promise<c.Address> {
    const r = StackReader.fromGetMethod(
      1,
      await provider.get('get_nft_address_by_index', [
        { type: 'int', value: itemIndex },
      ]),
    );
    return r.readSlice().loadAddress();
  }

  async getRoyaltyParams(provider: ContractProvider): Promise<RoyaltyParams> {
    const r = StackReader.fromGetMethod(
      3,
      await provider.get('royalty_params', []),
    );
    return {
      $: 'RoyaltyParams',
      numerator: r.readBigInt(),
      denominator: r.readBigInt(),
      royaltyAddress: r.readSlice().loadAddress(),
    };
  }

  async getNftContent(
    provider: ContractProvider,
    _itemIndex: bigint,
    individualNftContent: string,
  ): Promise<CellRef<OffchainMetadataReply>> {
    const r = StackReader.fromGetMethod(
      1,
      await provider.get('get_nft_content', [
        { type: 'int', value: _itemIndex },
        {
          type: 'cell',
          cell: beginCell().storeStringTail(individualNftContent).endCell(),
        },
      ]),
    );
    return r.readCellRef<OffchainMetadataReply>(
      OffchainMetadataReply.fromSlice,
    );
  }
}
