import {Cell} from "@ton/core"

import type {V3RunGetMethodResponse, V3RunGetMethodStackEntry} from "../api/types"

export interface VestingData {
  readonly vestingStartTime: number
  readonly vestingTotalDuration: number
  readonly unlockPeriod: number
  readonly cliffDuration: number
  readonly vestingTotalAmount: bigint
  readonly vestingSenderAddress: string
  readonly ownerAddress: string
}

export interface VestingPeriod {
  readonly number: number
  readonly startTime: number
  readonly payoutTime: number
  readonly amount: bigint
  readonly cumulativeAmount: bigint
  readonly status: "unlocked" | "next" | "locked"
}

export interface VestingSchedule {
  readonly totalPeriods: number
  readonly unlockedPeriods: number
  readonly unlockedAmount: bigint
  readonly lockedAmount: bigint
  readonly nextPayoutTime?: number
  readonly periods: readonly VestingPeriod[]
}

export function parseVestingData(response: V3RunGetMethodResponse): VestingData {
  if (response.exit_code !== 0) {
    throw new Error(`get_vesting_data exited with code ${response.exit_code}.`)
  }
  if (response.stack.length !== 8) {
    throw new TypeError(
      `get_vesting_data returned ${response.stack.length} stack values instead of 8.`,
    )
  }
  if (response.stack[7]?.type !== "cell") {
    throw new TypeError("Vesting field `whitelist` is not a cell stack value.")
  }

  return {
    vestingStartTime: readSafeStackInteger(response.stack[0], "vestingStartTime"),
    vestingTotalDuration: readSafeStackInteger(response.stack[1], "vestingTotalDuration"),
    unlockPeriod: readSafeStackInteger(response.stack[2], "unlockPeriod"),
    cliffDuration: readSafeStackInteger(response.stack[3], "cliffDuration"),
    vestingTotalAmount: readStackBigInt(response.stack[4], "vestingTotalAmount"),
    vestingSenderAddress: readStackAddress(response.stack[5], "vestingSenderAddress"),
    ownerAddress: readStackAddress(response.stack[6], "ownerAddress"),
  }
}

export function buildVestingSchedule(data: VestingData, nowSeconds: number): VestingSchedule {
  const {vestingStartTime, vestingTotalDuration, unlockPeriod, cliffDuration, vestingTotalAmount} =
    data
  if (
    vestingTotalDuration <= 0 ||
    unlockPeriod <= 0 ||
    vestingTotalDuration % unlockPeriod !== 0 ||
    cliffDuration < 0 ||
    cliffDuration >= vestingTotalDuration ||
    cliffDuration % unlockPeriod !== 0
  ) {
    throw new RangeError("Vesting contract returned an invalid unlock schedule.")
  }
  if (vestingTotalAmount < 0n) {
    throw new RangeError("Vesting contract returned a negative total amount.")
  }

  const totalPeriods = vestingTotalDuration / unlockPeriod
  if (!Number.isSafeInteger(totalPeriods) || totalPeriods <= 0) {
    throw new RangeError("Vesting contract returned too many unlock periods.")
  }

  const currentTime = Number.isFinite(nowSeconds) ? Math.floor(nowSeconds) : 0
  const cliffEndTime = vestingStartTime + cliffDuration
  const elapsedPeriods =
    currentTime < cliffEndTime ? 0 : Math.floor((currentTime - vestingStartTime) / unlockPeriod)
  const unlockedPeriods = Math.min(totalPeriods, Math.max(0, elapsedPeriods))
  const cliffPeriods = cliffDuration / unlockPeriod
  const nextPeriodNumber = unlockedPeriods + 1
  const nextPayoutTime =
    nextPeriodNumber <= totalPeriods
      ? vestingStartTime + Math.max(nextPeriodNumber, cliffPeriods) * unlockPeriod
      : undefined
  const periodCount = BigInt(totalPeriods)
  let previousCumulativeAmount = 0n

  const periods = Array.from({length: totalPeriods}, (_, index): VestingPeriod => {
    const number = index + 1
    const cumulativeAmount = (vestingTotalAmount * BigInt(number)) / periodCount
    const payoutTime = vestingStartTime + Math.max(number, cliffPeriods) * unlockPeriod
    const period: VestingPeriod = {
      number,
      startTime: vestingStartTime + index * unlockPeriod,
      payoutTime,
      amount: cumulativeAmount - previousCumulativeAmount,
      cumulativeAmount,
      status:
        number <= unlockedPeriods ? "unlocked" : payoutTime === nextPayoutTime ? "next" : "locked",
    }
    previousCumulativeAmount = cumulativeAmount
    return period
  })
  const unlockedAmount = periods[unlockedPeriods - 1]?.cumulativeAmount ?? 0n

  return {
    totalPeriods,
    unlockedPeriods,
    unlockedAmount,
    lockedAmount: vestingTotalAmount - unlockedAmount,
    nextPayoutTime,
    periods,
  }
}

function readStackBigInt(entry: V3RunGetMethodStackEntry | undefined, name: string): bigint {
  if (entry?.type !== "num") {
    throw new TypeError(`Vesting field \`${name}\` is not a numeric stack value.`)
  }

  if (typeof entry.value === "number" && Number.isSafeInteger(entry.value)) {
    return BigInt(entry.value)
  }
  if (typeof entry.value === "string") {
    const normalized = entry.value.trim()
    const negativeHex = normalized.match(/^-0x([0-9a-f]+)$/i)
    if (/^-?0x[0-9a-f]+$/i.test(normalized) || /^-?\d+$/.test(normalized)) {
      return negativeHex ? -BigInt(`0x${negativeHex[1]}`) : BigInt(normalized)
    }
  }

  throw new TypeError(`Vesting field \`${name}\` is not an integer.`)
}

function readSafeStackInteger(entry: V3RunGetMethodStackEntry | undefined, name: string): number {
  const value = Number(readStackBigInt(entry, name))
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError(`Vesting field \`${name}\` is outside the supported range.`)
  }
  return value
}

function readStackAddress(entry: V3RunGetMethodStackEntry | undefined, name: string): string {
  if (entry?.type !== "cell" || typeof entry.value !== "string") {
    throw new TypeError(`Vesting field \`${name}\` is not an address cell.`)
  }

  try {
    return Cell.fromBase64(entry.value).beginParse().loadAddress().toRawString()
  } catch {
    throw new TypeError(`Vesting field \`${name}\` does not contain a valid address.`)
  }
}
