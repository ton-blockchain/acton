// AUTO-GENERATED, do not edit
// it's a TypeScript wrapper for a NftItem contract in Tolk
/* eslint-disable */

import * as c from '@ton/core';
import { beginCell, ContractProvider, Sender, SendMode } from '@ton/core';

// ————————————————————————————————————————————
//   predefined types and functions
//

type RemainingBitsAndRefs = c.Slice

type StoreCallback<T> = (obj: T, b: c.Builder) => void
type LoadCallback<T> = (s: c.Slice) => T

export type CellRef<T> = {
    ref: T
}

function makeCellFrom<T>(self: T, storeFn_T: StoreCallback<T>): c.Cell {
    let b = beginCell();
    storeFn_T(self, b);
    return b.endCell();
}

function loadAndCheckPrefix32(s: c.Slice, expected: number, structName: string): void {
    let prefix = s.loadUint(32);
    if (prefix !== expected) {
        throw new Error(`Incorrect prefix for '${structName}': expected 0x${expected.toString(16).padStart(8, '0')}, got 0x${prefix.toString(16).padStart(8, '0')}`);
    }
}

function formatPrefix(prefixNum: number, prefixLen: number): string {
    return prefixLen % 4 ? `0b${prefixNum.toString(2).padStart(prefixLen, '0')}` : `0x${prefixNum.toString(16).padStart(prefixLen / 4, '0')}`;
}

function loadAndCheckPrefix(s: c.Slice, expected: number, prefixLen: number, structName: string): void {
    let prefix = s.loadUint(prefixLen);
    if (prefix !== expected) {
        throw new Error(`Incorrect prefix for '${structName}': expected ${formatPrefix(expected, prefixLen)}, got ${formatPrefix(prefix, prefixLen)}`);
    }
}

function lookupPrefix(s: c.Slice, expected: number, prefixLen: number): boolean {
    return s.remainingBits >= prefixLen && s.preloadUint(prefixLen) === expected;
}

function throwNonePrefixMatch(fieldPath: string): never {
    throw new Error(`Incorrect prefix for '${fieldPath}': none of variants matched`);
}

function storeCellRef<T>(cell: CellRef<T>, b: c.Builder, storeFn_T: StoreCallback<T>): void {
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

function storeTolkNullable<T>(v: T | null, b: c.Builder, storeFn_T: StoreCallback<T>): void {
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
    constructor(private tuple: c.TupleItem[]) {
    }

    static fromGetMethod(expectedN: number, getMethodResult: { stack: c.TupleReader }): StackReader {
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

    readSnakeString(): string {
        return this.readCell().beginParse().loadStringTail();
    }

    readNullable<T>(readFn_T: (r: StackReader) => T): T | null {
        if (this.tuple[0].type === 'null') {
            this.tuple.shift();
            return null;
        }
        return readFn_T(this);
    }

    readWideNullable<T>(stackW: number, readFn_T: (r: StackReader) => T): T | null {
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
}

// ————————————————————————————————————————————
//   auto-generated serializers to/from cells
//

type coins = bigint

type int8 = bigint
type int16 = bigint
type int32 = bigint
type int256 = bigint

type uint8 = bigint
type uint16 = bigint
type uint32 = bigint
type uint64 = bigint
type uint256 = bigint

/**
 > struct (0b0) PayloadInline {
 >     value: RemainingBitsAndRefs
 > }
 */
export interface PayloadInline {
    readonly $: 'PayloadInline'
    value: RemainingBitsAndRefs
}

export const PayloadInline = {
    PREFIX: 0b0,

    create(args: {
        value: RemainingBitsAndRefs
    }): PayloadInline {
        return {
            $: 'PayloadInline',
            ...args
        }
    },
    fromSlice(s: c.Slice): PayloadInline {
        loadAndCheckPrefix(s, 0b0, 1, 'PayloadInline');
        return {
            $: 'PayloadInline',
            value: loadTolkRemaining(s),
        }
    },
    store(self: PayloadInline, b: c.Builder): void {
        b.storeUint(0b0, 1);
        storeTolkRemaining(self.value, b);
    },
    toCell(self: PayloadInline): c.Cell {
        return makeCellFrom<PayloadInline>(self, PayloadInline.store);
    }
}

/**
 > struct (0b1) PayloadInRef {
 >     value: Cell<RemainingBitsAndRefs>
 > }
 */
export interface PayloadInRef {
    readonly $: 'PayloadInRef'
    value: CellRef<RemainingBitsAndRefs>
}

export const PayloadInRef = {
    PREFIX: 0b1,

    create(args: {
        value: CellRef<RemainingBitsAndRefs>
    }): PayloadInRef {
        return {
            $: 'PayloadInRef',
            ...args
        }
    },
    fromSlice(s: c.Slice): PayloadInRef {
        loadAndCheckPrefix(s, 0b1, 1, 'PayloadInRef');
        return {
            $: 'PayloadInRef',
            value: loadCellRef<RemainingBitsAndRefs>(s, loadTolkRemaining),
        }
    },
    store(self: PayloadInRef, b: c.Builder): void {
        b.storeUint(0b1, 1);
        storeCellRef<RemainingBitsAndRefs>(self.value, b, storeTolkRemaining);
    },
    toCell(self: PayloadInRef): c.Cell {
        return makeCellFrom<PayloadInRef>(self, PayloadInRef.store);
    }
}

/**
 > type Payload = PayloadInline | PayloadInRef
 */
export type Payload =
    | PayloadInline
    | PayloadInRef

export const Payload = {
    fromSlice(s: c.Slice): Payload {
        return s.loadBoolean() ? PayloadInRef.fromSlice(s) : PayloadInline.fromSlice(s);
    },
    store(self: Payload, b: c.Builder): void {
        switch (self.$) {
            case 'PayloadInline':
                PayloadInline.store(self, b);
                break;
            case 'PayloadInRef':
                PayloadInRef.store(self, b);
                break;
        }
    },
    toCell(self: Payload): c.Cell {
        return makeCellFrom<Payload>(self, Payload.store);
    }
}

/**
 > struct (0x5fcc3d14) AskToChangeOwnership {
 >     queryId: uint64
 >     newOwnerAddress: address
 >     sendExcessesTo: address?
 >     customPayload: cell?
 >     forwardTonAmount: coins
 >     forwardPayload: Payload
 > }
 */
export interface AskToChangeOwnership {
    readonly $: 'AskToChangeOwnership'
    queryId: uint64
    newOwnerAddress: c.Address
    sendExcessesTo: c.Address | null
    customPayload: c.Cell | null
    forwardTonAmount: coins
    forwardPayload: Payload
}

export const AskToChangeOwnership = {
    PREFIX: 0x5fcc3d14,

    create(args: {
        queryId: uint64
        newOwnerAddress: c.Address
        sendExcessesTo: c.Address | null
        customPayload: c.Cell | null
        forwardTonAmount: coins
        forwardPayload: Payload
    }): AskToChangeOwnership {
        return {
            $: 'AskToChangeOwnership',
            ...args
        }
    },
    fromSlice(s: c.Slice): AskToChangeOwnership {
        loadAndCheckPrefix32(s, 0x5fcc3d14, 'AskToChangeOwnership');
        return {
            $: 'AskToChangeOwnership',
            queryId: s.loadUintBig(64),
            newOwnerAddress: s.loadAddress(),
            sendExcessesTo: s.loadMaybeAddress(),
            customPayload: s.loadBoolean() ? s.loadRef() : null,
            forwardTonAmount: s.loadCoins(),
            forwardPayload: Payload.fromSlice(s),
        }
    },
    store(self: AskToChangeOwnership, b: c.Builder): void {
        b.storeUint(0x5fcc3d14, 32);
        b.storeUint(self.queryId, 64);
        b.storeAddress(self.newOwnerAddress);
        b.storeAddress(self.sendExcessesTo);
        storeTolkNullable<c.Cell>(self.customPayload, b,
            (v,b) => b.storeRef(v)
        );
        b.storeCoins(self.forwardTonAmount);
        Payload.store(self.forwardPayload, b);
    },
    toCell(self: AskToChangeOwnership): c.Cell {
        return makeCellFrom<AskToChangeOwnership>(self, AskToChangeOwnership.store);
    }
}

/**
 > struct (0x2fcb26a2) RequestStaticData {
 >     queryId: uint64
 > }
 */
export interface RequestStaticData {
    readonly $: 'RequestStaticData'
    queryId: uint64
}

export const RequestStaticData = {
    PREFIX: 0x2fcb26a2,

    create(args: {
        queryId: uint64
    }): RequestStaticData {
        return {
            $: 'RequestStaticData',
            ...args
        }
    },
    fromSlice(s: c.Slice): RequestStaticData {
        loadAndCheckPrefix32(s, 0x2fcb26a2, 'RequestStaticData');
        return {
            $: 'RequestStaticData',
            queryId: s.loadUintBig(64),
        }
    },
    store(self: RequestStaticData, b: c.Builder): void {
        b.storeUint(0x2fcb26a2, 32);
        b.storeUint(self.queryId, 64);
    },
    toCell(self: RequestStaticData): c.Cell {
        return makeCellFrom<RequestStaticData>(self, RequestStaticData.store);
    }
}

/**
 > struct NftItemStorage {
 >     itemIndex: uint64
 >     collectionAddress: address
 >     ownerAddress: address
 >     content: string
 > }
 */
export interface NftItemStorage {
    readonly $: 'NftItemStorage'
    itemIndex: uint64
    collectionAddress: c.Address
    ownerAddress: c.Address
    content: string
}

export const NftItemStorage = {
    create(args: {
        itemIndex: uint64
        collectionAddress: c.Address
        ownerAddress: c.Address
        content: string
    }): NftItemStorage {
        return {
            $: 'NftItemStorage',
            ...args
        }
    },
    fromSlice(s: c.Slice): NftItemStorage {
        return {
            $: 'NftItemStorage',
            itemIndex: s.loadUintBig(64),
            collectionAddress: s.loadAddress(),
            ownerAddress: s.loadAddress(),
            content: s.loadStringRefTail(),
        }
    },
    store(self: NftItemStorage, b: c.Builder): void {
        b.storeUint(self.itemIndex, 64);
        b.storeAddress(self.collectionAddress);
        b.storeAddress(self.ownerAddress);
        b.storeStringRefTail(self.content);
    },
    toCell(self: NftItemStorage): c.Cell {
        return makeCellFrom<NftItemStorage>(self, NftItemStorage.store);
    }
}

/**
 > struct NftItemStorageNotInitialized {
 >     itemIndex: uint64
 >     collectionAddress: address
 > }
 */
export interface NftItemStorageNotInitialized {
    readonly $: 'NftItemStorageNotInitialized'
    itemIndex: uint64
    collectionAddress: c.Address
}

export const NftItemStorageNotInitialized = {
    create(args: {
        itemIndex: uint64
        collectionAddress: c.Address
    }): NftItemStorageNotInitialized {
        return {
            $: 'NftItemStorageNotInitialized',
            ...args
        }
    },
    fromSlice(s: c.Slice): NftItemStorageNotInitialized {
        return {
            $: 'NftItemStorageNotInitialized',
            itemIndex: s.loadUintBig(64),
            collectionAddress: s.loadAddress(),
        }
    },
    store(self: NftItemStorageNotInitialized, b: c.Builder): void {
        b.storeUint(self.itemIndex, 64);
        b.storeAddress(self.collectionAddress);
    },
    toCell(self: NftItemStorageNotInitialized): c.Cell {
        return makeCellFrom<NftItemStorageNotInitialized>(self, NftItemStorageNotInitialized.store);
    }
}

/**
 > struct (0x05138d91) NotificationForNewOwner {
 >     queryId: uint64
 >     oldOwnerAddress: address
 >     payload: Payload
 > }
 */
export interface NotificationForNewOwner {
    readonly $: 'NotificationForNewOwner'
    queryId: uint64
    oldOwnerAddress: c.Address
    payload: Payload
}

export const NotificationForNewOwner = {
    PREFIX: 0x05138d91,

    create(args: {
        queryId: uint64
        oldOwnerAddress: c.Address
        payload: Payload
    }): NotificationForNewOwner {
        return {
            $: 'NotificationForNewOwner',
            ...args
        }
    },
    fromSlice(s: c.Slice): NotificationForNewOwner {
        loadAndCheckPrefix32(s, 0x05138d91, 'NotificationForNewOwner');
        return {
            $: 'NotificationForNewOwner',
            queryId: s.loadUintBig(64),
            oldOwnerAddress: s.loadAddress(),
            payload: Payload.fromSlice(s),
        }
    },
    store(self: NotificationForNewOwner, b: c.Builder): void {
        b.storeUint(0x05138d91, 32);
        b.storeUint(self.queryId, 64);
        b.storeAddress(self.oldOwnerAddress);
        Payload.store(self.payload, b);
    },
    toCell(self: NotificationForNewOwner): c.Cell {
        return makeCellFrom<NotificationForNewOwner>(self, NotificationForNewOwner.store);
    }
}

/**
 > struct (0xd53276db) ReturnExcessesBack {
 >     queryId: uint64
 > }
 */
export interface ReturnExcessesBack {
    readonly $: 'ReturnExcessesBack'
    queryId: uint64
}

export const ReturnExcessesBack = {
    PREFIX: 0xd53276db,

    create(args: {
        queryId: uint64
    }): ReturnExcessesBack {
        return {
            $: 'ReturnExcessesBack',
            ...args
        }
    },
    fromSlice(s: c.Slice): ReturnExcessesBack {
        loadAndCheckPrefix32(s, 0xd53276db, 'ReturnExcessesBack');
        return {
            $: 'ReturnExcessesBack',
            queryId: s.loadUintBig(64),
        }
    },
    store(self: ReturnExcessesBack, b: c.Builder): void {
        b.storeUint(0xd53276db, 32);
        b.storeUint(self.queryId, 64);
    },
    toCell(self: ReturnExcessesBack): c.Cell {
        return makeCellFrom<ReturnExcessesBack>(self, ReturnExcessesBack.store);
    }
}

/**
 > struct (0x8b771735) ResponseStaticData {
 >     queryId: uint64
 >     itemIndex: uint256
 >     collectionAddress: address
 > }
 */
export interface ResponseStaticData {
    readonly $: 'ResponseStaticData'
    queryId: uint64
    itemIndex: uint256
    collectionAddress: c.Address
}

export const ResponseStaticData = {
    PREFIX: 0x8b771735,

    create(args: {
        queryId: uint64
        itemIndex: uint256
        collectionAddress: c.Address
    }): ResponseStaticData {
        return {
            $: 'ResponseStaticData',
            ...args
        }
    },
    fromSlice(s: c.Slice): ResponseStaticData {
        loadAndCheckPrefix32(s, 0x8b771735, 'ResponseStaticData');
        return {
            $: 'ResponseStaticData',
            queryId: s.loadUintBig(64),
            itemIndex: s.loadUintBig(256),
            collectionAddress: s.loadAddress(),
        }
    },
    store(self: ResponseStaticData, b: c.Builder): void {
        b.storeUint(0x8b771735, 32);
        b.storeUint(self.queryId, 64);
        b.storeUint(self.itemIndex, 256);
        b.storeAddress(self.collectionAddress);
    },
    toCell(self: ResponseStaticData): c.Cell {
        return makeCellFrom<ResponseStaticData>(self, ResponseStaticData.store);
    }
}

/**
 > struct NftDataReply {
 >     isInitialized: bool
 >     itemIndex: int
 >     collectionAddress: address
 >     ownerAddress: address?
 >     content: string?
 > }
 */
export interface NftDataReply {
    readonly $: 'NftDataReply'
    isInitialized: boolean
    itemIndex: bigint
    collectionAddress: c.Address
    ownerAddress: c.Address | null /* = null */
    content: string | null /* = null */
}

export const NftDataReply = {
    create(args: {
        isInitialized: boolean
        itemIndex: bigint
        collectionAddress: c.Address
        ownerAddress?: c.Address | null /* = null */
        content?: string | null /* = null */
    }): NftDataReply {
        return {
            $: 'NftDataReply',
            ownerAddress: null,
            content: null,
            ...args
        }
    },
    fromSlice(s: c.Slice): NftDataReply {
        throw new Error(`Can't unpack 'NftDataReply' from cell, because 'NftDataReply.itemIndex' is 'int' (not int32/uint64/etc.)`);
    },
    store(self: NftDataReply, b: c.Builder): void {
        throw new Error(`Can't pack 'NftDataReply' to cell, because 'self.itemIndex' is 'int' (not int32/uint64/etc.)`);
    },
    toCell(self: NftDataReply): c.Cell {
        return makeCellFrom<NftDataReply>(self, NftDataReply.store);
    }
}

// ————————————————————————————————————————————
//    class NftItem
//

interface ExtraSendOptions {
    bounce?: boolean                    // default: false
    sendMode?: SendMode                 // default: SendMode.PAY_GAS_SEPARATELY
    extraCurrencies?: c.ExtraCurrency   // default: empty dict
}

interface DeployedAddrOptions {
    workchain?: number                  // default: 0 (basechain)
    toShard?: { fixedPrefixLength: number; closeTo: c.Address }
    overrideContractCode?: c.Cell
}

function calculateDeployedAddress(code: c.Cell, data: c.Cell, options: DeployedAddrOptions): c.Address {
    const stateInitCell = beginCell().store(c.storeStateInit({
        code,
        data,
        splitDepth: options.toShard?.fixedPrefixLength,
        special: null,          // todo will somebody need special?
        libraries: null,        // todo will somebody need libraries?
    })).endCell();

    let addrHash = stateInitCell.hash();
    if (options.toShard) {
        const shardDepth = options.toShard.fixedPrefixLength;
        addrHash = beginCell()  // todo any way to do it better? N bits from closeTo + 256-N from stateInitCell
            .storeBits(new c.BitString(options.toShard.closeTo.hash, 0, shardDepth))
            .storeBits(new c.BitString(stateInitCell.hash(), shardDepth, 256 - shardDepth))
            .endCell()
            .beginParse().loadBuffer(32);
    }

    return new c.Address(options.workchain ?? 0, addrHash);
}

export class NftItem implements c.Contract {
    static CodeCell = c.Cell.fromBase64('te6ccgECBwEAAWsAART/APSkE/S88sgLAQIBYgIDAeTQ+JHyQO1E0NM/+kggxwCOHDD4kiHHBfLhlQL6SNTRAsjLPxP6UhL6UszJ7VTg+kjXTCTXLCL+Yeik4wJsIdcsIX5ZNRSOITMC1ws/+JLIz4UI+lKCEIt3FzXPC47LP8v/+lLJgED7AOBfA4QPAccA8vQEADWhH5/aiaGmf/SQQY4BKmDgstrbwfSRrpj+qmEC6jX4kiLHBfLhkQTTP/pI+lD0AfoA1ywCk4EAgpzXLAaS8j/h10yBAIPiJPpEMPLRTfiTcPg6+CdvEIIK+vCAoSSUU0Ggod4lbpExmCX6RDDy0U2i4iDC//LhkiOTN18D4w0gbpMTXwPjDgLIyz/6UvpSzMntVAUGAGLIz5AUTjZGJ88LPxj6UoEAgli6k8+BzpPPg8ziycjPhQhSQPpSWPoCcc8LaszJcfsAADLIz4UI+lJQA/oCghDVMnbbzwuKyz/JcfsA');

    static Errors = {
        'Errors.InvalidWorkchain': 333,
        'Errors.NotFromOwner': 401,
        'Errors.TooSmallRestAmount': 402,
        'Errors.NotFromCollection': 405,
    }

    readonly address: c.Address
    readonly init?: { code: c.Cell, data: c.Cell }

    private constructor(address: c.Address, init?: { code: c.Cell, data: c.Cell }) {
        this.address = address;
        this.init = init;
    }

    static fromAddress(address: c.Address) {
        return new NftItem(address);
    }

    static fromStorage(emptyStorage: {
        itemIndex: uint64
        collectionAddress: c.Address
    }, deployedOptions?: DeployedAddrOptions) {
        const initialState = {
            code: deployedOptions?.overrideContractCode ?? NftItem.CodeCell,
            data: NftItemStorageNotInitialized.toCell(NftItemStorageNotInitialized.create(emptyStorage)),
        };
        const address = calculateDeployedAddress(initialState.code, initialState.data, deployedOptions ?? {});
        return new NftItem(address, initialState);
    }

    static createCellOfAskToChangeOwnership(body: {
        queryId: uint64
        newOwnerAddress: c.Address
        sendExcessesTo: c.Address | null
        customPayload: c.Cell | null
        forwardTonAmount: coins
        forwardPayload: Payload
    }) {
        return AskToChangeOwnership.toCell(AskToChangeOwnership.create(body));
    }

    static createCellOfRequestStaticData(body: {
        queryId: uint64
    }) {
        return RequestStaticData.toCell(RequestStaticData.create(body));
    }

    async sendDeploy(provider: ContractProvider, via: Sender, msgValue: coins, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: c.Cell.EMPTY,
            ...extraOptions
        });
    }

    async sendAskToChangeOwnership(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        queryId: uint64
        newOwnerAddress: c.Address
        sendExcessesTo: c.Address | null
        customPayload: c.Cell | null
        forwardTonAmount: coins
        forwardPayload: Payload
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: AskToChangeOwnership.toCell(AskToChangeOwnership.create(body)),
            ...extraOptions
        });
    }

    async sendRequestStaticData(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        queryId: uint64
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: RequestStaticData.toCell(RequestStaticData.create(body)),
            ...extraOptions
        });
    }

    async getNftData(provider: ContractProvider): Promise<NftDataReply> {
        const r = StackReader.fromGetMethod(5, await provider.get('get_nft_data', []));
        return ({
            $: 'NftDataReply',
            isInitialized: r.readBoolean(),
            itemIndex: r.readBigInt(),
            collectionAddress: r.readSlice().loadAddress(),
            ownerAddress: r.readNullable<c.Address>(
                (r) => r.readSlice().loadAddress()
            ),
            content: r.readNullable<string>(
                (r) => r.readSnakeString()
            ),
        });
    }
}
