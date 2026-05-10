// AUTO-GENERATED, do not edit
// It's a TypeScript wrapper for a WalletV5 contract in Tolk.
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

function storeTolkBitsN(v: c.Slice, nBits: number, b: c.Builder): void {
    if (v.remainingBits !== nBits) { throw new Error(`expected ${nBits} bits, got ${v.remainingBits}`); }
    if (v.remainingRefs !== 0) { throw new Error(`expected 0 refs, got ${v.remainingRefs}`); }
    b.storeSlice(v);
}

function loadTolkBitsN(s: c.Slice, nBits: number): c.Slice {
    return new c.Slice(new c.BitReader(s.loadBits(nBits)), []);
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
        if (item?.type === itemType) {
            return item as ItemT;
        }
        throw new Error(`not '${itemType}' on a stack`);
    }

    private popCellLike(): c.Cell {
        const item = this.tuple.shift();
        if (item && (item.type === 'cell' || item.type === 'slice' || item.type === 'builder')) {
            return item.cell;
        }
        throw new Error(`not cell/slice on a stack`);
    }

    readBigInt(): bigint {
        return this.popExpecting<c.TupleItemInt>('int').value;
    }

    readBoolean(): boolean {
        return this.popExpecting<c.TupleItemInt>('int').value !== 0n;
    }

    readCell(): c.Cell {
        return this.popCellLike();
    }

    readSlice(): c.Slice {
        return this.popCellLike().beginParse();
    }

    readDictionary<K extends c.DictionaryKeyTypes, V>(keySerializer: c.DictionaryKey<K>, valueSerializer: c.DictionaryValue<V>): c.Dictionary<K, V> {
        if (this.tuple[0].type === 'null') {
            this.tuple.shift();
            return c.Dictionary.empty<K, V>(keySerializer, valueSerializer);
        }
        return c.Dictionary.loadDirect<K, V>(keySerializer, valueSerializer, this.readCell());
    }
}

// ————————————————————————————————————————————
//   auto-generated serializers to/from cells
//

type coins = bigint

type uint32 = bigint
type uint64 = bigint
type uint256 = bigint

type bits512 = c.Slice

/**
 > type OutActionsCell = cell
 */
export type OutActionsCell = c.Cell

export const OutActionsCell = {
    fromSlice(s: c.Slice): OutActionsCell {
        return s.loadRef();
    },
    store(self: OutActionsCell, b: c.Builder): void {
        b.storeRef(self);
    },
    toCell(self: OutActionsCell): c.Cell {
        return makeCellFrom<OutActionsCell>(self, OutActionsCell.store);
    }
}

/**
 > type SnakedExtraActions = RemainingBitsAndRefs
 */
export type SnakedExtraActions = RemainingBitsAndRefs

export const SnakedExtraActions = {
    fromSlice(s: c.Slice): SnakedExtraActions {
        return loadTolkRemaining(s);
    },
    store(self: SnakedExtraActions, b: c.Builder): void {
        storeTolkRemaining(self, b);
    },
    toCell(self: SnakedExtraActions): c.Cell {
        return makeCellFrom<SnakedExtraActions>(self, SnakedExtraActions.store);
    }
}

/**
 > struct (0x6578746e) ExtensionActionRequest {
 >     queryId: uint64
 >     outActions: OutActionsCell?
 >     hasExtraActions: bool
 >     extraActions: SnakedExtraActions
 > }
 */
export interface ExtensionActionRequest {
    readonly $: 'ExtensionActionRequest'
    queryId: uint64
    outActions: OutActionsCell | null
    hasExtraActions: boolean
    extraActions: SnakedExtraActions
}

export const ExtensionActionRequest = {
    PREFIX: 0x6578746e,

    create(args: {
        queryId: uint64
        outActions: OutActionsCell | null
        hasExtraActions: boolean
        extraActions: SnakedExtraActions
    }): ExtensionActionRequest {
        return {
            $: 'ExtensionActionRequest',
            ...args
        }
    },
    fromSlice(s: c.Slice): ExtensionActionRequest {
        loadAndCheckPrefix32(s, 0x6578746e, 'ExtensionActionRequest');
        return {
            $: 'ExtensionActionRequest',
            queryId: s.loadUintBig(64),
            outActions: s.loadBoolean() ? OutActionsCell.fromSlice(s) : null,
            hasExtraActions: s.loadBoolean(),
            extraActions: SnakedExtraActions.fromSlice(s),
        }
    },
    store(self: ExtensionActionRequest, b: c.Builder): void {
        b.storeUint(0x6578746e, 32);
        b.storeUint(self.queryId, 64);
        storeTolkNullable<OutActionsCell>(self.outActions, b, OutActionsCell.store);
        b.storeBit(self.hasExtraActions);
        SnakedExtraActions.store(self.extraActions, b);
    },
    toCell(self: ExtensionActionRequest): c.Cell {
        return makeCellFrom<ExtensionActionRequest>(self, ExtensionActionRequest.store);
    }
}

/**
 > struct (0x73696e74) InternalSignedRequest {
 >     walletId: uint32
 >     validUntil: uint32
 >     seqno: uint32
 >     outActions: OutActionsCell?
 >     hasExtraActions: bool
 >     extraActions: SnakedExtraActions
 > }
 */
export interface InternalSignedRequest {
    readonly $: 'InternalSignedRequest'
    walletId: uint32
    validUntil: uint32
    seqno: uint32
    outActions: OutActionsCell | null
    hasExtraActions: boolean
    extraActions: SnakedExtraActions
}

export const InternalSignedRequest = {
    PREFIX: 0x73696e74,

    create(args: {
        walletId: uint32
        validUntil: uint32
        seqno: uint32
        outActions: OutActionsCell | null
        hasExtraActions: boolean
        extraActions: SnakedExtraActions
    }): InternalSignedRequest {
        return {
            $: 'InternalSignedRequest',
            ...args
        }
    },
    fromSlice(s: c.Slice): InternalSignedRequest {
        loadAndCheckPrefix32(s, 0x73696e74, 'InternalSignedRequest');
        return {
            $: 'InternalSignedRequest',
            walletId: s.loadUintBig(32),
            validUntil: s.loadUintBig(32),
            seqno: s.loadUintBig(32),
            outActions: s.loadBoolean() ? OutActionsCell.fromSlice(s) : null,
            hasExtraActions: s.loadBoolean(),
            extraActions: SnakedExtraActions.fromSlice(s),
        }
    },
    store(self: InternalSignedRequest, b: c.Builder): void {
        b.storeUint(0x73696e74, 32);
        b.storeUint(self.walletId, 32);
        b.storeUint(self.validUntil, 32);
        b.storeUint(self.seqno, 32);
        storeTolkNullable<OutActionsCell>(self.outActions, b, OutActionsCell.store);
        b.storeBit(self.hasExtraActions);
        SnakedExtraActions.store(self.extraActions, b);
    },
    toCell(self: InternalSignedRequest): c.Cell {
        return makeCellFrom<InternalSignedRequest>(self, InternalSignedRequest.store);
    }
}

/**
 > struct ExternalMsgSchema {
 >     signedData: ExternalSignedRequest
 >     signature: bits512
 > }
 */
export interface ExternalMsgSchema {
    readonly $: 'ExternalMsgSchema'
    signedData: ExternalSignedRequest
    signature: bits512
}

export const ExternalMsgSchema = {
    create(args: {
        signedData: ExternalSignedRequest
        signature: bits512
    }): ExternalMsgSchema {
        return {
            $: 'ExternalMsgSchema',
            ...args
        }
    },
    fromSlice(s: c.Slice): ExternalMsgSchema {
        return {
            $: 'ExternalMsgSchema',
            signedData: ExternalSignedRequest.fromSlice(s),
            signature: loadTolkBitsN(s, 512),
        }
    },
    store(self: ExternalMsgSchema, b: c.Builder): void {
        ExternalSignedRequest.store(self.signedData, b);
        storeTolkBitsN(self.signature, 512, b);
    },
    toCell(self: ExternalMsgSchema): c.Cell {
        return makeCellFrom<ExternalMsgSchema>(self, ExternalMsgSchema.store);
    }
}

/**
 > struct (0x7369676e) ExternalSignedRequest {
 >     walletId: uint32
 >     validUntil: uint32
 >     seqno: uint32
 >     outActions: OutActionsCell?
 >     hasExtraActions: bool
 >     extraActions: SnakedExtraActions
 > }
 */
export interface ExternalSignedRequest {
    readonly $: 'ExternalSignedRequest'
    walletId: uint32
    validUntil: uint32
    seqno: uint32
    outActions: OutActionsCell | null
    hasExtraActions: boolean
    extraActions: SnakedExtraActions
}

export const ExternalSignedRequest = {
    PREFIX: 0x7369676e,

    create(args: {
        walletId: uint32
        validUntil: uint32
        seqno: uint32
        outActions: OutActionsCell | null
        hasExtraActions: boolean
        extraActions: SnakedExtraActions
    }): ExternalSignedRequest {
        return {
            $: 'ExternalSignedRequest',
            ...args
        }
    },
    fromSlice(s: c.Slice): ExternalSignedRequest {
        loadAndCheckPrefix32(s, 0x7369676e, 'ExternalSignedRequest');
        return {
            $: 'ExternalSignedRequest',
            walletId: s.loadUintBig(32),
            validUntil: s.loadUintBig(32),
            seqno: s.loadUintBig(32),
            outActions: s.loadBoolean() ? OutActionsCell.fromSlice(s) : null,
            hasExtraActions: s.loadBoolean(),
            extraActions: SnakedExtraActions.fromSlice(s),
        }
    },
    store(self: ExternalSignedRequest, b: c.Builder): void {
        b.storeUint(0x7369676e, 32);
        b.storeUint(self.walletId, 32);
        b.storeUint(self.validUntil, 32);
        b.storeUint(self.seqno, 32);
        storeTolkNullable<OutActionsCell>(self.outActions, b, OutActionsCell.store);
        b.storeBit(self.hasExtraActions);
        SnakedExtraActions.store(self.extraActions, b);
    },
    toCell(self: ExternalSignedRequest): c.Cell {
        return makeCellFrom<ExternalSignedRequest>(self, ExternalSignedRequest.store);
    }
}

/**
 > type ExtensionsDict = map<uint256, bool>
 */
export type ExtensionsDict = c.Dictionary<uint256, boolean>

export const ExtensionsDict = {
    fromSlice(s: c.Slice): ExtensionsDict {
        return c.Dictionary.load<uint256, boolean>(c.Dictionary.Keys.BigUint(256), c.Dictionary.Values.Bool(), s);
    },
    store(self: ExtensionsDict, b: c.Builder): void {
        b.storeDict<uint256, boolean>(self, c.Dictionary.Keys.BigUint(256), c.Dictionary.Values.Bool());
    },
    toCell(self: ExtensionsDict): c.Cell {
        return makeCellFrom<ExtensionsDict>(self, ExtensionsDict.store);
    }
}

/**
 > struct Storage {
 >     isSignatureAllowed: bool
 >     seqno: uint32
 >     subwalletId: uint32
 >     publicKey: uint256
 >     extensions: ExtensionsDict
 > }
 */
export interface Storage {
    readonly $: 'Storage'
    isSignatureAllowed: boolean
    seqno: uint32
    subwalletId: uint32
    publicKey: uint256
    extensions: ExtensionsDict
}

export const Storage = {
    create(args: {
        isSignatureAllowed: boolean
        seqno: uint32
        subwalletId: uint32
        publicKey: uint256
        extensions: ExtensionsDict
    }): Storage {
        return {
            $: 'Storage',
            ...args
        }
    },
    fromSlice(s: c.Slice): Storage {
        return {
            $: 'Storage',
            isSignatureAllowed: s.loadBoolean(),
            seqno: s.loadUintBig(32),
            subwalletId: s.loadUintBig(32),
            publicKey: s.loadUintBig(256),
            extensions: ExtensionsDict.fromSlice(s),
        }
    },
    store(self: Storage, b: c.Builder): void {
        b.storeBit(self.isSignatureAllowed);
        b.storeUint(self.seqno, 32);
        b.storeUint(self.subwalletId, 32);
        b.storeUint(self.publicKey, 256);
        ExtensionsDict.store(self.extensions, b);
    },
    toCell(self: Storage): c.Cell {
        return makeCellFrom<Storage>(self, Storage.store);
    }
}

// ————————————————————————————————————————————
//    class WalletV5
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
        special: null,
        libraries: null,
    })).endCell();

    let addrHash = stateInitCell.hash();
    if (options.toShard) {
        const shardDepth = options.toShard.fixedPrefixLength;
        addrHash = beginCell()
            .storeBits(new c.BitString(options.toShard.closeTo.hash, 0, shardDepth))
            .storeBits(new c.BitString(stateInitCell.hash(), shardDepth, 256 - shardDepth))
            .endCell()
            .beginParse().loadBuffer(32);
    }

    return new c.Address(options.workchain ?? 0, addrHash);
}

export class WalletV5 implements c.Contract {
    static CodeCell = c.Cell.fromBase64('te6ccgECEwEAAxcAART/APSkE/S88sgLAQIBIAIDAgFIBAUC4PIggwjXIgGDCNcjINcsI5tLO3Ty4IrTH9Mf0x/0BNIA7UTQ0gDTHyDTH9P/9AUMyM75FkDd+RDy4IcJbhKx8uCEUUS68uCFUFe68uCGAvgjvPLgiPgApMjPg8sfE87J7VT4D3AibpEy4w6Ogts84FsQEQP40CDXLCObS3Okj3Ah10mBAoC+kVvhIYMI1yICgwjXI+1E0NIA0x8g0x/T//QFBsjO+RZAiPkQkl8G4QNuErHy4IQC0x/TH9Mf9ATSAFE3uvLghVBHuvLghgH4I7zy4IgDpMjPg8sfEs7J7VRwIW6RMeMOAo6DAds8kVvi4AYRBwIBIAgJAGhwItc5MI4iINdLwALy4JPAKPLgk9csIHYeQ2zy4JPXTNc5MAGkIccAEuYwhAe78uCTAe1VAfQx1ywjK8OjdI7u+JL6RPgo+kQwWL2RW+DtRNCBAUHXIfQFgwf0Dm+hMZEw4dM/MfQE0gB/I26RM440cCTXOTCOIiDXS8AC8uCTwCjy4JPXLCB2HkNs8uCT10zXOTABpCHHABLmMIQHu/LgkwPtVeIBjoMB2zyRW+LgMBECASAKCwAZvl8PaiaECAoOuQ+gLAIBbgwNAgFIDg8AF63OdqJoaaAY64X/wAAXrx32omhpkBjrhY/AABezJftRNDTADHXCx+AAEbJi+1E0NcKAIAB2cCPXOTCOKSDXS8AC8uCTwCjy4JPXLCB2HkNs8uCT1NcLB3Kw8uCJ1zkwAaQhxwAS5jCEB7vy4JMC7VUBkO2i7fvrIdcsCBSOLfpIMPpE+Cj6RDBYuvLgke1E0IEBQdcY9AXIz4NAM4MH9FPy4IsByM70AMntVOMOIddKk1vbMeEB10zQARIA7NcsCByONvpIMPpE+Cj6RDBYuvLgke1E0NIAgQFA1xj0BRODB/Rb8uCMIZUgbvLQkN8ByMoAEs70AMntVI431ywIJJPywI3hIfLgku1E0AHXCgAB0gAggQFA1yH0BVEjvfLgjyKRMZUBbvLQjuIByMoAzsntVOI=');

    static Errors = {
        'Errors.SignatureDisabled': 132,
        'Errors.InvalidSeqno': 133,
        'Errors.InvalidWalletId': 134,
        'Errors.InvalidSignature': 135,
        'Errors.Expired': 136,
        'Errors.ExternalSendMessageMustHaveIgnoreErrorsSendMode': 137,
        'Errors.InvalidMessageOperation': 138,
        'Errors.AddExtension': 139,
        'Errors.RemoveExtension': 140,
        'Errors.UnsupportedAction': 141,
        'Errors.DisableSignatureWhenExtensionsIsEmpty': 142,
        'Errors.ThisSignatureModeAlreadySet': 143,
        'Errors.RemoveLastExtensionWhenSignatureDisabled': 144,
        'Errors.ExtensionWrongWorkchain': 145,
        'Errors.OnlyExtensionCanChangeSignatureMode': 146,
        'Errors.InvalidC5': 147,
    }

    readonly address: c.Address
    readonly init: { code: c.Cell, data: c.Cell } | undefined

    protected constructor(address: c.Address, init?: { code: c.Cell, data: c.Cell }) {
        this.address = address;
        this.init = init;
    }

    static fromAddress(address: c.Address) {
        return new WalletV5(address);
    }

    static fromStorage(emptyStorage: {
        isSignatureAllowed: boolean
        seqno: uint32
        subwalletId: uint32
        publicKey: uint256
        extensions: ExtensionsDict
    }, deployedOptions?: DeployedAddrOptions) {
        const initialState = {
            code: deployedOptions?.overrideContractCode ?? WalletV5.CodeCell,
            data: Storage.toCell(Storage.create(emptyStorage)),
        };
        const address = calculateDeployedAddress(initialState.code, initialState.data, deployedOptions ?? {});
        return new WalletV5(address, initialState);
    }

    static createCellOfExtensionActionRequest(body: {
        queryId: uint64
        outActions: OutActionsCell | null
        hasExtraActions: boolean
        extraActions: SnakedExtraActions
    }) {
        return ExtensionActionRequest.toCell(ExtensionActionRequest.create(body));
    }

    static createCellOfInternalSignedRequest(body: {
        walletId: uint32
        validUntil: uint32
        seqno: uint32
        outActions: OutActionsCell | null
        hasExtraActions: boolean
        extraActions: SnakedExtraActions
    }) {
        return InternalSignedRequest.toCell(InternalSignedRequest.create(body));
    }

    async sendDeploy(provider: ContractProvider, via: Sender, msgValue: coins, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: c.Cell.EMPTY,
            ...extraOptions
        });
    }

    async sendExtensionActionRequest(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        queryId: uint64
        outActions: OutActionsCell | null
        hasExtraActions: boolean
        extraActions: SnakedExtraActions
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: ExtensionActionRequest.toCell(ExtensionActionRequest.create(body)),
            ...extraOptions
        });
    }

    async sendInternalSignedRequest(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        walletId: uint32
        validUntil: uint32
        seqno: uint32
        outActions: OutActionsCell | null
        hasExtraActions: boolean
        extraActions: SnakedExtraActions
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: InternalSignedRequest.toCell(InternalSignedRequest.create(body)),
            ...extraOptions
        });
    }

    async getIsSignatureAllowed(provider: ContractProvider): Promise<boolean> {
        const r = StackReader.fromGetMethod(1, await provider.get('is_signature_allowed', []));
        return r.readBoolean();
    }

    async getSeqno(provider: ContractProvider): Promise<bigint> {
        const r = StackReader.fromGetMethod(1, await provider.get('seqno', []));
        return r.readBigInt();
    }

    async getSubwalletId(provider: ContractProvider): Promise<bigint> {
        const r = StackReader.fromGetMethod(1, await provider.get('get_subwallet_id', []));
        return r.readBigInt();
    }

    async getPublicKey(provider: ContractProvider): Promise<bigint> {
        const r = StackReader.fromGetMethod(1, await provider.get('get_public_key', []));
        return r.readBigInt();
    }

    async getExtensions(provider: ContractProvider): Promise<c.Dictionary<uint256, boolean>> {
        const r = StackReader.fromGetMethod(1, await provider.get('get_extensions', []));
        return r.readDictionary<uint256, boolean>(c.Dictionary.Keys.BigUint(256), c.Dictionary.Values.Bool());
    }
}
