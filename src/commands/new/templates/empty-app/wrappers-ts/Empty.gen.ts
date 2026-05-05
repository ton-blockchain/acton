// AUTO-GENERATED, do not edit
// it's a TypeScript wrapper for a Empty contract in Tolk
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
 > type AllowedMessage = ChangeOwner
 */
export type AllowedMessage = ChangeOwner

export const AllowedMessage = {
    fromSlice(s: c.Slice): AllowedMessage {
        return ChangeOwner.fromSlice(s);
    },
    store(self: AllowedMessage, b: c.Builder): void {
        ChangeOwner.store(self, b);
    },
    toCell(self: AllowedMessage): c.Cell {
        return makeCellFrom<AllowedMessage>(self, AllowedMessage.store);
    }
}

/**
 > struct Storage {
 >     owner: address
 > }
 */
export interface Storage {
    readonly $: 'Storage'
    owner: c.Address
}

export const Storage = {
    create(args: {
        owner: c.Address
    }): Storage {
        return {
            $: 'Storage',
            ...args
        }
    },
    fromSlice(s: c.Slice): Storage {
        return {
            $: 'Storage',
            owner: s.loadAddress(),
        }
    },
    store(self: Storage, b: c.Builder): void {
        b.storeAddress(self.owner);
    },
    toCell(self: Storage): c.Cell {
        return makeCellFrom<Storage>(self, Storage.store);
    }
}

/**
 > struct (0x2ce05111) ChangeOwner {
 >     newOwner: address
 > }
 */
export interface ChangeOwner {
    readonly $: 'ChangeOwner'
    newOwner: c.Address
}

export const ChangeOwner = {
    PREFIX: 0x2ce05111,

    create(args: {
        newOwner: c.Address
    }): ChangeOwner {
        return {
            $: 'ChangeOwner',
            ...args
        }
    },
    fromSlice(s: c.Slice): ChangeOwner {
        loadAndCheckPrefix32(s, 0x2ce05111, 'ChangeOwner');
        return {
            $: 'ChangeOwner',
            newOwner: s.loadAddress(),
        }
    },
    store(self: ChangeOwner, b: c.Builder): void {
        b.storeUint(0x2ce05111, 32);
        b.storeAddress(self.newOwner);
    },
    toCell(self: ChangeOwner): c.Cell {
        return makeCellFrom<ChangeOwner>(self, ChangeOwner.store);
    }
}

// ————————————————————————————————————————————
//    class Empty
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

export class Empty implements c.Contract {
    static CodeCell = c.Cell.fromBase64('te6ccgEBBAEATwABFP8A9KQT9LzyyAsBAgFiAgMAYND4kZEw4CDXLCFnAoiMjhcx7UTQ+kgw+JLHBfLgZPpIMMj6UsntVOAwhA8BxwDy9AARoIo72omh9JBh');

    static Errors = {
        'Errors.NotOwner': 100,
        'Errors.InvalidMessage': 65535,
    }

    readonly address: c.Address
    readonly init?: { code: c.Cell, data: c.Cell }

    private constructor(address: c.Address, init?: { code: c.Cell, data: c.Cell }) {
        this.address = address;
        this.init = init;
    }

    static fromAddress(address: c.Address) {
        return new Empty(address);
    }

    static fromStorage(emptyStorage: {
        owner: c.Address
    }, deployedOptions?: DeployedAddrOptions) {
        const initialState = {
            code: deployedOptions?.overrideContractCode ?? Empty.CodeCell,
            data: Storage.toCell(Storage.create(emptyStorage)),
        };
        const address = calculateDeployedAddress(initialState.code, initialState.data, deployedOptions ?? {});
        return new Empty(address, initialState);
    }

    static createCellOfAllowedMessage(body: AllowedMessage) {
        return AllowedMessage.toCell(body);
    }

    async sendDeploy(provider: ContractProvider, via: Sender, msgValue: coins, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: c.Cell.EMPTY,
            ...extraOptions
        });
    }

    async sendAllowedMessage(provider: ContractProvider, via: Sender, msgValue: coins, body: AllowedMessage, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: AllowedMessage.toCell(body),
            ...extraOptions
        });
    }

    async getOwner(provider: ContractProvider): Promise<c.Address> {
        const r = StackReader.fromGetMethod(1, await provider.get('owner', []));
        return r.readSlice().loadAddress();
    }
}
