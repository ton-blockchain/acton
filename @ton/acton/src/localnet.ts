import {Buffer} from "node:buffer"
import type {ChildProcess} from "node:child_process"
import {mkdtempSync, rmSync} from "node:fs"
import {tmpdir} from "node:os"
import path from "node:path"
import process from "node:process"

import {
  Address,
  beginCell,
  Cell,
  type Contract,
  type ContractGetMethodResult,
  type ContractProvider,
  type ContractState,
  loadTransaction,
  type Message,
  type StateInit,
  storeMessage,
  toNano,
  type Transaction,
  type TupleItem,
  TupleReader,
} from "@ton/core"

import {addressFromSeed, formatAddress, isContract, parseAddress} from "./address.js"
import {createContractHandle, type ContractHandle} from "./contract.js"
import {ActonError, LocalnetApiError, errorMessage} from "./errors.js"
import {LocalnetHttpClient} from "./http.js"
import {startLocalnetProcess, waitForChildExit} from "./process.js"
import {LocalnetContractProvider} from "./provider.js"
import {LocalnetSender} from "./sender.js"
import {toContractState} from "./state.js"
import {legacyJsonToTupleItem, tupleItemToLegacyJson} from "./stack.js"
import {registerAfterAll, registerAfterEach} from "./test-lifecycle.js"
import type {
  AccountInfoResult,
  CloseLocalnetOptions,
  EmulateTraceResult,
  LocalnetCoverageRecord,
  LocalnetNodeInfo,
  LocalnetTraceRecord,
  LocalnetTreasuryRecord,
  LocalnetOptions,
  RawTransaction,
  RunGetMethodResult,
  SendBocResult,
  StartLocalnetOptions,
  TrackTransactionsOptions,
  TransactionsOptions,
  WaitUntilReadyOptions,
} from "./types.js"
import {delay, normalizeEndpoint} from "./utils.js"

const DEFAULT_STARTUP_TIMEOUT_MS = 10 * 1000
const DEFAULT_POLL_INTERVAL_MS = 100
const DEFAULT_CLOSE_TIMEOUT_MS = 5 * 1000
const DEFAULT_TRACKED_TRANSACTIONS_LIMIT = 32

export class Localnet {
  readonly endpoint: string

  private readonly child?: ChildProcess
  private readonly coverageRecords: LocalnetCoverageRecord[] = []
  private readonly traceRecords: LocalnetTraceRecord[] = []
  private readonly treasuryRecords = new Map<string, LocalnetTreasuryRecord>()
  private readonly http: LocalnetHttpClient
  private unregisterAutoClose?: () => void
  private unregisterAutoReset?: () => void

  constructor(options: LocalnetOptions = {}, child?: ChildProcess, autoClose = false) {
    this.endpoint = normalizeEndpoint(options.endpoint ?? "http://127.0.0.1:5411")
    this.child = child
    this.http = new LocalnetHttpClient(this.endpoint)
    if (child && autoClose) {
      this.unregisterAutoClose = registerAutoClose(child)
    }
  }

  static connect(options: LocalnetOptions = {}): Localnet {
    return new Localnet(options)
  }

  static async start(options: StartLocalnetOptions = {}): Promise<Localnet> {
    const started = await startLocalnetProcess(options)
    const localnet = new Localnet(
      {endpoint: started.endpoint},
      started.child,
      options.autoClose ?? true,
    )
    await localnet.waitUntilReady({
      timeoutMs: options.startupTimeoutMs,
      pollIntervalMs: options.pollIntervalMs,
    })
    if (options.autoClose ?? true) {
      await localnet.enableTestAutoClose()
    }
    if (options.autoReset ?? true) {
      await localnet.enableAutoReset()
    }
    return localnet
  }

  provider(contract: Contract): ContractProvider
  provider(address: Address | string, init?: StateInit | null): ContractProvider
  provider(target: Contract | Address | string, init?: StateInit | null): ContractProvider {
    if (isContract(target)) {
      return new LocalnetContractProvider(this, target.address, target.init ?? null)
    }
    return new LocalnetContractProvider(this, parseAddress(target), init ?? null)
  }

  contract<T extends Contract>(contract: T): ContractHandle<T> {
    return createContractHandle(contract, this.provider(contract))
  }

  sender(address: Address | string): LocalnetSender {
    return new LocalnetSender(this, parseAddress(address))
  }

  treasury(name: string, workchain = 0): LocalnetSender {
    const address = addressFromSeed(name, workchain)
    this.treasuryRecords.set(`${workchain}:${name}`, {
      address: formatAddress(address),
      name,
    })
    return this.sender(address)
  }

  async nodeInfo(): Promise<LocalnetNodeInfo> {
    return this.http.getJson<LocalnetNodeInfo>("/acton_nodeInfo")
  }

  async waitUntilReady(options: WaitUntilReadyOptions = {}): Promise<void> {
    const timeoutMs = options.timeoutMs ?? DEFAULT_STARTUP_TIMEOUT_MS
    const pollIntervalMs = options.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS
    const deadline = Date.now() + timeoutMs
    let lastError: unknown

    while (Date.now() <= deadline) {
      if (this.child?.exitCode !== null && this.child?.exitCode !== undefined) {
        throw new ActonError("acton localnet exited before it became ready")
      }

      try {
        await this.nodeInfo()
        return
      } catch (error) {
        lastError = error
        await delay(pollIntervalMs)
      }
    }

    throw new ActonError(
      `Timed out waiting for localnet at ${this.endpoint}: ${errorMessage(lastError)}`,
    )
  }

  async close(options: CloseLocalnetOptions = {}): Promise<void> {
    this.unregisterAutoReset?.()
    this.unregisterAutoReset = undefined
    this.unregisterAutoClose?.()
    this.unregisterAutoClose = undefined

    const child = this.child
    if (!child || child.exitCode !== null) {
      return
    }

    const timeoutMs = options.timeoutMs ?? DEFAULT_CLOSE_TIMEOUT_MS
    child.kill(options.signal ?? "SIGTERM")

    await Promise.race([
      waitForChildExit(child),
      delay(timeoutMs).then(() => {
        if (child.exitCode === null) {
          child.kill("SIGKILL")
        }
      }),
    ])
  }

  async sendBoc(boc: string | Cell | Buffer): Promise<SendBocResult> {
    const encoded =
      typeof boc === "string"
        ? boc
        : Buffer.isBuffer(boc)
          ? boc.toString("base64")
          : boc.toBoc().toString("base64")

    if (this.collectCoverage() || this.collectTraces()) {
      await this.captureMessageDiagnostics(encoded)
    }

    return this.http.postJson<SendBocResult>("/api/v2/sendBocReturnHash", {boc: encoded})
  }

  async sendMessage(message: Message): Promise<SendBocResult> {
    return this.sendBoc(beginCell().store(storeMessage(message)).endCell())
  }

  async transactions(
    address: Address | string,
    options: TransactionsOptions = {},
  ): Promise<Transaction[]> {
    const query = new URLSearchParams({
      address: formatAddress(parseAddress(address)),
      limit: String(options.limit ?? 10),
    })

    if (options.lt !== undefined) {
      query.set("lt", options.lt.toString())
    }
    if (options.hash !== undefined) {
      query.set("hash", Buffer.isBuffer(options.hash) ? options.hash.toString("hex") : options.hash)
    }
    if (options.toLt !== undefined) {
      query.set("to_lt", options.toLt.toString())
    }

    const rows = await this.http.getJson<readonly RawTransaction[]>(
      `/api/v2/getTransactions?${query}`,
    )
    return rows.map(row => loadTransaction(Cell.fromBase64(row.data).beginParse()))
  }

  async trackTransactions(
    address: Address | string,
    action: () => unknown,
    options: TrackTransactionsOptions = {},
  ): Promise<Transaction[]> {
    const parsedAddress = parseAddress(address)
    const state = await this.getAccountState(parsedAddress)
    const previousLt = state.last?.lt ?? 0n

    await action()

    const transactions = await this.transactions(parsedAddress, {
      limit: options.limit ?? DEFAULT_TRACKED_TRANSACTIONS_LIMIT,
    })
    return transactions.filter(transaction => transaction.lt > previousLt).reverse()
  }

  async airdrop(address: Address | string, amount: bigint | string = toNano("100")): Promise<void> {
    const amountNanotons = typeof amount === "string" ? toNano(amount) : amount
    if (amountNanotons > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new ActonError(
        "airdrop amount must fit into a JSON-safe integer for the localnet faucet",
      )
    }

    await this.http.postJson<unknown>("/acton_fundAccount", {
      address: formatAddress(parseAddress(address)),
      amount: Number(amountNanotons),
    })
  }

  async dumpState(path: string): Promise<void> {
    await this.http.postJson<unknown>("/acton_dumpState", {path})
  }

  async loadState(path: string): Promise<void> {
    await this.http.postJson<unknown>("/acton_loadState", {path})
  }

  async getAccountState(address: Address): Promise<ContractState> {
    const query = new URLSearchParams({address: formatAddress(address)})
    try {
      const info = await this.http.getJson<AccountInfoResult>(
        `/api/v2/getAddressInformation?${query}`,
      )
      return toContractState(info)
    } catch (error) {
      if (isPreGenesisStateError(error)) {
        return {
          balance: 0n,
          extracurrency: null,
          last: null,
          state: {type: "uninit"},
        }
      }
      throw error
    }
  }

  async runGetMethod(
    address: Address,
    name: string | number,
    args: readonly TupleItem[],
  ): Promise<ContractGetMethodResult> {
    const coverageState = this.collectCoverage() ? await this.getAccountState(address) : undefined
    const result = await this.http.postJson<RunGetMethodResult>("/api/v2/runGetMethod", {
      address: formatAddress(address),
      method: name,
      stack: args.map(item => tupleItemToLegacyJson(item)),
    })

    if (result.exit_code !== 0) {
      throw new ActonError(`Get method ${String(name)} failed with exit code ${result.exit_code}`)
    }

    if (coverageState?.state.type === "active" && coverageState.state.code && result.vm_log) {
      this.coverageRecords.push({
        code: coverageState.state.code.toString("base64"),
        vmLog: result.vm_log,
      })
    }

    return {
      stack: new TupleReader(result.stack.map(item => legacyJsonToTupleItem(item))),
      gasUsed: result.gas_used === undefined ? undefined : BigInt(result.gas_used),
      logs: result.vm_log,
    }
  }

  consumeCoverageRecords(): readonly LocalnetCoverageRecord[] {
    const records = [...this.coverageRecords]
    this.coverageRecords.length = 0
    return records
  }

  consumeTraceRecords(): readonly LocalnetTraceRecord[] {
    const records = [...this.traceRecords]
    this.traceRecords.length = 0
    return records
  }

  consumeTreasuryRecords(): readonly LocalnetTreasuryRecord[] {
    const records = [...this.treasuryRecords.values()]
    this.treasuryRecords.clear()
    return records
  }

  private collectCoverage(): boolean {
    return process.env.ACTON_NODE_COVERAGE === "1"
  }

  private collectTraces(): boolean {
    return process.env.ACTON_NODE_TRACE === "1"
  }

  private async captureMessageDiagnostics(boc: string): Promise<void> {
    const result = await this.http.postRawJson<EmulateTraceResult>("/api/emulate/v1/emulateTrace", {
      boc,
      include_code_data: true,
    })

    if (this.collectTraces()) {
      this.traceRecords.push(...(result.acton_trace_records ?? []))
    }

    if (this.collectCoverage() && result.vm_log) {
      for (const code of Object.values(result.code_cells ?? {})) {
        this.coverageRecords.push({code, vmLog: result.vm_log})
      }
    }
  }

  private async enableAutoReset(): Promise<void> {
    const snapshot = createStateSnapshotPath()
    let active = true

    const registered = await registerAfterEach(async () => {
      if (active) {
        await this.loadState(snapshot.path)
      }
    })

    if (!registered) {
      snapshot.cleanup()
      return
    }

    await this.dumpState(snapshot.path)
    this.unregisterAutoReset = () => {
      active = false
      snapshot.cleanup()
    }
  }

  private async enableTestAutoClose(): Promise<void> {
    await registerAfterAll(async () => {
      await this.close()
    })
  }
}

function isPreGenesisStateError(error: unknown): boolean {
  return error instanceof LocalnetApiError && error.message === "Block 0 not found"
}

function registerAutoClose(child: ChildProcess): () => void {
  child.unref()
  let registered = true

  const killChild = (): void => {
    if (child.exitCode === null && !child.killed) {
      child.kill("SIGTERM")
    }
  }

  const unregister = (): void => {
    if (!registered) {
      return
    }
    registered = false
    process.off("beforeExit", killChild)
    process.off("exit", killChild)
  }

  child.once("exit", unregister)
  process.once("beforeExit", killChild)
  process.once("exit", killChild)
  return unregister
}

function createStateSnapshotPath(): {readonly path: string; readonly cleanup: () => void} {
  const directory = mkdtempSync(path.join(tmpdir(), "acton-localnet-state-"))
  return {
    path: path.join(directory, "initial-state.json"),
    cleanup: () => {
      rmSync(directory, {force: true, recursive: true})
    },
  }
}
