import type {V3RunGetMethodResponse, V3RunGetMethodStackEntry} from "../api/types"

export interface LockerData {
  readonly totalCoinsLocked: bigint
  readonly totalReward: bigint
  readonly depositsEndTime: number
  readonly vestingStartTime: number
  readonly vestingTotalDuration: number
  readonly unlockPeriod: number
}

export interface LockerPayment {
  readonly number: number
  readonly unlockTime: number
  readonly amount: bigint
  readonly cumulativeAmount: bigint
  readonly status: "unlocked" | "next" | "locked"
}

export interface LockerSchedule {
  readonly totalAmount: bigint
  readonly totalPeriods: number
  readonly unlockedPeriods: number
  readonly unlockedAmount: bigint
  readonly nextPayment?: LockerPayment
  readonly payments: readonly LockerPayment[]
}

export function parseLockerData(response: V3RunGetMethodResponse): LockerData {
  if (response.exit_code !== 0) {
    throw new Error(`get_locker_data exited with code ${response.exit_code}.`)
  }
  if (response.stack.length !== 6) {
    throw new TypeError(
      `get_locker_data returned ${response.stack.length} stack values instead of 6.`,
    )
  }

  return {
    totalCoinsLocked: readStackBigInt(response.stack[0], "totalCoinsLocked"),
    totalReward: readStackBigInt(response.stack[1], "totalReward"),
    depositsEndTime: readSafeStackInteger(response.stack[2], "depositsEndTime"),
    vestingStartTime: readSafeStackInteger(response.stack[3], "vestingStartTime"),
    vestingTotalDuration: readSafeStackInteger(response.stack[4], "vestingTotalDuration"),
    unlockPeriod: readSafeStackInteger(response.stack[5], "unlockPeriod"),
  }
}

export function buildLockerSchedule(data: LockerData, nowSeconds: number): LockerSchedule {
  const {vestingStartTime, vestingTotalDuration, unlockPeriod} = data
  if (vestingTotalDuration <= 0 || unlockPeriod <= 0 || vestingTotalDuration % unlockPeriod !== 0) {
    throw new RangeError("Locker returned an invalid unlock schedule.")
  }

  const totalPeriods = vestingTotalDuration / unlockPeriod
  if (!Number.isSafeInteger(totalPeriods) || totalPeriods <= 0) {
    throw new RangeError("Locker returned too many unlock periods.")
  }

  const currentTime = Number.isFinite(nowSeconds) ? Math.floor(nowSeconds) : 0
  const elapsedPeriods =
    currentTime < vestingStartTime + unlockPeriod
      ? 0
      : Math.floor((currentTime - vestingStartTime) / unlockPeriod)
  const unlockedPeriods = Math.min(totalPeriods, Math.max(0, elapsedPeriods))
  const totalAmount = data.totalCoinsLocked + data.totalReward
  const periodCount = BigInt(totalPeriods)
  let previousCumulativeAmount = 0n

  const payments = Array.from({length: totalPeriods}, (_, index): LockerPayment => {
    const number = index + 1
    const cumulativeAmount = (totalAmount * BigInt(number)) / periodCount
    const payment: LockerPayment = {
      number,
      unlockTime: vestingStartTime + number * unlockPeriod,
      amount: cumulativeAmount - previousCumulativeAmount,
      cumulativeAmount,
      status:
        number <= unlockedPeriods ? "unlocked" : number === unlockedPeriods + 1 ? "next" : "locked",
    }
    previousCumulativeAmount = cumulativeAmount
    return payment
  })

  return {
    totalAmount,
    totalPeriods,
    unlockedPeriods,
    unlockedAmount: payments[unlockedPeriods - 1]?.cumulativeAmount ?? 0n,
    nextPayment: payments[unlockedPeriods],
    payments,
  }
}

function readStackBigInt(entry: V3RunGetMethodStackEntry | undefined, name: string): bigint {
  if (entry?.type !== "num") {
    throw new TypeError(`Locker field \`${name}\` is not a numeric stack value.`)
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

  throw new TypeError(`Locker field \`${name}\` is not an integer.`)
}

function readSafeStackInteger(entry: V3RunGetMethodStackEntry | undefined, name: string): number {
  const value = Number(readStackBigInt(entry, name))
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError(`Locker field \`${name}\` is outside the supported range.`)
  }
  return value
}
