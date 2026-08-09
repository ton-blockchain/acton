import {describe, expect, test} from "bun:test"

import type {V3RunGetMethodResponse} from "../src/api/types"
import {buildLockerSchedule, parseLockerData} from "../src/components/lockerSchedule"

const lockerResponse = {
  gas_used: 1,
  exit_code: 0,
  stack: [
    {type: "num", value: "0xe584082dc82c5c9"},
    {type: "num", value: "0x3f0049d42b6e400"},
    {type: "num", value: "1698019200"},
    {type: "num", value: "1760227200"},
    {type: "num", value: "93312000"},
    {type: "num", value: "2592000"},
  ],
  vm_log: "",
} satisfies V3RunGetMethodResponse

describe("Locker schedule", () => {
  test("parses the six-value get_locker_data stack", () => {
    expect(parseLockerData(lockerResponse)).toEqual({
      totalCoinsLocked: 1_033_647_045_271_012_809n,
      totalReward: 283_731_850_000_000_000n,
      depositsEndTime: 1_698_019_200,
      vestingStartTime: 1_760_227_200,
      vestingTotalDuration: 93_312_000,
      unlockPeriod: 2_592_000,
    })
  })

  test("unlocks after complete periods and preserves the full amount", () => {
    const data = parseLockerData(lockerResponse)
    const beforeFirstPayment = buildLockerSchedule(
      data,
      data.vestingStartTime + data.unlockPeriod - 1,
    )
    const afterNinePayments = buildLockerSchedule(
      data,
      data.vestingStartTime + data.unlockPeriod * 9,
    )
    const completed = buildLockerSchedule(data, data.vestingStartTime + data.vestingTotalDuration)

    expect(beforeFirstPayment.unlockedPeriods).toBe(0)
    expect(beforeFirstPayment.nextPayment?.number).toBe(1)
    expect(afterNinePayments.unlockedPeriods).toBe(9)
    expect(afterNinePayments.nextPayment?.number).toBe(10)
    expect(completed.unlockedPeriods).toBe(36)
    expect(completed.nextPayment).toBeUndefined()
    expect(completed.payments.reduce((total, payment) => total + payment.amount, 0n)).toBe(
      completed.totalAmount,
    )
    expect(completed.unlockedAmount).toBe(completed.totalAmount)
  })

  test("rejects a malformed get-method result", () => {
    expect(() =>
      parseLockerData({
        ...lockerResponse,
        stack: lockerResponse.stack.slice(0, 5),
      }),
    ).toThrow("5 stack values instead of 6")
    expect(() =>
      parseLockerData({
        ...lockerResponse,
        stack: [{type: "cell", value: ""}, ...lockerResponse.stack.slice(1)],
      }),
    ).toThrow("totalCoinsLocked")
  })
})
