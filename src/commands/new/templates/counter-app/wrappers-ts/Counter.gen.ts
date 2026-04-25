// AUTO-GENERATED, do not edit
// it's a TypeScript wrapper for a Counter contract in Tolk
/* eslint-disable */

import * as c from '@ton/core';
import { beginCell, ContractProvider, Sender, SendMode } from '@ton/core';

// ————————————————————————————————————————————
//   predefined types and functions
//

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
type uint256 = bigint

/**
 > struct (0x7e8764ef) IncreaseCounter {
 >     increaseBy: uint32
 > }
 */
export interface IncreaseCounter {
    readonly $: 'IncreaseCounter'
    increaseBy: uint32
}

export const IncreaseCounter = {
    PREFIX: 0x7e8764ef,

    create(args: {
        increaseBy: uint32
    }): IncreaseCounter {
        return {
            $: 'IncreaseCounter',
            ...args
        }
    },
    fromSlice(s: c.Slice): IncreaseCounter {
        loadAndCheckPrefix32(s, 0x7e8764ef, 'IncreaseCounter');
        return {
            $: 'IncreaseCounter',
            increaseBy: s.loadUintBig(32),
        }
    },
    store(self: IncreaseCounter, b: c.Builder): void {
        b.storeUint(0x7e8764ef, 32);
        b.storeUint(self.increaseBy, 32);
    },
    toCell(self: IncreaseCounter): c.Cell {
        return makeCellFrom<IncreaseCounter>(self, IncreaseCounter.store);
    }
}

/**
 > struct (0x283b4c3f) DecreaseCounter {
 >     decreaseBy: uint32
 > }
 */
export interface DecreaseCounter {
    readonly $: 'DecreaseCounter'
    decreaseBy: uint32
}

export const DecreaseCounter = {
    PREFIX: 0x283b4c3f,

    create(args: {
        decreaseBy: uint32
    }): DecreaseCounter {
        return {
            $: 'DecreaseCounter',
            ...args
        }
    },
    fromSlice(s: c.Slice): DecreaseCounter {
        loadAndCheckPrefix32(s, 0x283b4c3f, 'DecreaseCounter');
        return {
            $: 'DecreaseCounter',
            decreaseBy: s.loadUintBig(32),
        }
    },
    store(self: DecreaseCounter, b: c.Builder): void {
        b.storeUint(0x283b4c3f, 32);
        b.storeUint(self.decreaseBy, 32);
    },
    toCell(self: DecreaseCounter): c.Cell {
        return makeCellFrom<DecreaseCounter>(self, DecreaseCounter.store);
    }
}

/**
 > struct (0x3a752f06) ResetCounter {
 > }
 */
export interface ResetCounter {
    readonly $: 'ResetCounter'
}

export const ResetCounter = {
    PREFIX: 0x3a752f06,

    create(): ResetCounter {
        return {
            $: 'ResetCounter',
        }
    },
    fromSlice(s: c.Slice): ResetCounter {
        loadAndCheckPrefix32(s, 0x3a752f06, 'ResetCounter');
        return {
            $: 'ResetCounter',
        }
    },
    store(self: ResetCounter, b: c.Builder): void {
        b.storeUint(0x3a752f06, 32);
    },
    toCell(self: ResetCounter): c.Cell {
        return makeCellFrom<ResetCounter>(self, ResetCounter.store);
    }
}

/**
 > struct Storage {
 >     id: uint32
 >     owner: address
 >     counter: uint32
 > }
 */
export interface Storage {
    readonly $: 'Storage'
    id: uint32
    owner: c.Address
    counter: uint32
}

export const Storage = {
    create(args: {
        id: uint32
        owner: c.Address
        counter: uint32
    }): Storage {
        return {
            $: 'Storage',
            ...args
        }
    },
    fromSlice(s: c.Slice): Storage {
        return {
            $: 'Storage',
            id: s.loadUintBig(32),
            owner: s.loadAddress(),
            counter: s.loadUintBig(32),
        }
    },
    store(self: Storage, b: c.Builder): void {
        b.storeUint(self.id, 32);
        b.storeAddress(self.owner);
        b.storeUint(self.counter, 32);
    },
    toCell(self: Storage): c.Cell {
        return makeCellFrom<Storage>(self, Storage.store);
    }
}

// ————————————————————————————————————————————
//    class Counter
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

export class Counter implements c.Contract {
    static CodeCell = c.Cell.fromBase64('te6ccgEBBwEA1gABFP8A9KQT9LzyyAsBAgFiAgMB+ND4kZEw4CDXLCP0Oyd8jiQx7UTQ1h/6SNcLH/iSIscF8uBkA9cLHxOgAcjOEvpSyx/J7VTg1ywhQdph/I4sMe1E0NYf+kjXCx/4kiLHBfLgZAPXCx+BEAFTQb7y9BOhAcjOEvpSyx/J7VTg1ywh06l4NDHjAoQPAccA8vQEAgEgBQYAPjDtRNDWH/pIMPiSIccF8uBkAcjO+lLPkAAAAALJ7VQAF74o72omhpj5j9JBhAAdvlaHaiaGmPmP0kGOuFj8');

    static Errors = {
        'Errors.NotOwner': 100,
        'Errors.CounterUnderflow': 4097,
        'Errors.InvalidMessage': 65535,
    }

    readonly address: c.Address
    readonly init?: { code: c.Cell, data: c.Cell }

    private constructor(address: c.Address, init?: { code: c.Cell, data: c.Cell }) {
        this.address = address;
        this.init = init;
    }

    static fromAddress(address: c.Address) {
        return new Counter(address);
    }

    static fromStorage(emptyStorage: {
        id: uint32
        owner: c.Address
        counter: uint32
    }, deployedOptions?: DeployedAddrOptions) {
        const initialState = {
            code: deployedOptions?.overrideContractCode ?? Counter.CodeCell,
            data: Storage.toCell(Storage.create(emptyStorage)),
        };
        const address = calculateDeployedAddress(initialState.code, initialState.data, deployedOptions ?? {});
        return new Counter(address, initialState);
    }

    static createCellOfIncreaseCounter(body: {
        increaseBy: uint32
    }) {
        return IncreaseCounter.toCell(IncreaseCounter.create(body));
    }

    static createCellOfDecreaseCounter(body: {
        decreaseBy: uint32
    }) {
        return DecreaseCounter.toCell(DecreaseCounter.create(body));
    }

    static createCellOfResetCounter(body: {
    }) {
        return ResetCounter.toCell(ResetCounter.create());
    }

    async sendDeploy(provider: ContractProvider, via: Sender, msgValue: coins, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: c.Cell.EMPTY,
            ...extraOptions
        });
    }

    async sendIncreaseCounter(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        increaseBy: uint32
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: IncreaseCounter.toCell(IncreaseCounter.create(body)),
            ...extraOptions
        });
    }

    async sendDecreaseCounter(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        decreaseBy: uint32
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: DecreaseCounter.toCell(DecreaseCounter.create(body)),
            ...extraOptions
        });
    }

    async sendResetCounter(provider: ContractProvider, via: Sender, msgValue: coins, body: {
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: ResetCounter.toCell(ResetCounter.create()),
            ...extraOptions
        });
    }

    async getCurrentCounter(provider: ContractProvider): Promise<bigint> {
        const r = StackReader.fromGetMethod(1, await provider.get('currentCounter', []));
        return r.readBigInt();
    }

    async getOwner(provider: ContractProvider): Promise<c.Address> {
        const r = StackReader.fromGetMethod(1, await provider.get('owner', []));
        return r.readSlice().loadAddress();
    }
}
