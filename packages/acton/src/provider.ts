import {Buffer} from "node:buffer"

import {
  Address,
  Cell,
  comment,
  type Contract,
  type ContractGetMethodResult,
  type ContractProvider,
  type ContractState,
  external,
  openContract,
  type OpenedContract,
  type Sender,
  type StateInit,
  toNano,
  type Transaction,
  type TupleItem,
} from "@ton/core"

import type {Localnet} from "./localnet.js"

export class LocalnetContractProvider implements ContractProvider {
  constructor(
    private readonly localnet: Localnet,
    private readonly address: Address,
    private readonly init: StateInit | null,
  ) {}

  async getState(): Promise<ContractState> {
    return this.localnet.getAccountState(this.address)
  }

  async get(name: string | number, args: TupleItem[]): Promise<ContractGetMethodResult> {
    return this.localnet.runGetMethod(this.address, name, args)
  }

  async external(message: Cell): Promise<void> {
    const state = await this.getState()
    const init = this.initForState(state)

    return (await this.localnet.trackTransactions(this.address, async () => {
      await this.localnet.sendMessage(external({to: this.address, init, body: message}))
    })) as unknown as void
  }

  async internal(via: Sender, args: Parameters<ContractProvider["internal"]>[1]): Promise<void> {
    const value = typeof args.value === "string" ? toNano(args.value) : args.value
    const body = typeof args.body === "string" ? comment(args.body) : (args.body ?? undefined)
    const state = await this.getState()
    const init = this.initForState(state)

    return (await this.localnet.trackTransactions(this.address, async () => {
      await via.send({
        body,
        bounce: args.bounce ?? true,
        extracurrency: args.extracurrency,
        init,
        sendMode: args.sendMode,
        to: this.address,
        value,
      })
    })) as unknown as void
  }

  open<T extends Contract>(contract: T): OpenedContract<T> {
    return openContract(contract, ({address, init}) => this.localnet.provider(address, init))
  }

  async getTransactions(
    address: Address,
    lt: bigint,
    hash: Buffer,
    limit?: number,
  ): Promise<Transaction[]> {
    return this.localnet.transactions(address, {hash, limit, lt})
  }

  private initForState(state: ContractState): StateInit | undefined {
    return this.init && state.state.type !== "active" ? this.init : undefined
  }
}
