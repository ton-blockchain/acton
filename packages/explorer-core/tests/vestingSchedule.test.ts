import {describe, expect, test} from "bun:test"

import type {V3RunGetMethodResponse} from "../src/api/types"
import {buildVestingSchedule, parseVestingData} from "../src/components/vestingSchedule"

const vestingResponse = {
  gas_used: 2111,
  exit_code: 0,
  stack: [
    {type: "num", value: "0x64d0ce38"},
    {type: "num", value: "0x1da9c00"},
    {type: "num", value: "0x278d00"},
    {type: "num", value: "0x76a700"},
    {type: "num", value: "0x38d7ea4c68000"},
    {
      type: "cell",
      value: "te6cckEBAQEAJAAAQ4AE/9cpcsn5mDr47llJb9Mu8QjSAWqzJ4oYfZSTa5ra5PCGTVzv",
    },
    {
      type: "cell",
      value: "te6cckEBAQEAJAAAQ4AE/9cpcsn5mDr47llJb9Mu8QjSAWqzJ4oYfZSTa5ra5PCGTVzv",
    },
    {
      type: "cell",
      value: "te6cckEBAQEAJQAARaEf6qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq40c2pqg==",
    },
  ],
  vm_log: "",
} satisfies V3RunGetMethodResponse

describe("Vesting schedule", () => {
  test("parses get_vesting_data including controlling addresses", () => {
    expect(parseVestingData(vestingResponse)).toEqual({
      vestingStartTime: 1_691_405_880,
      vestingTotalDuration: 31_104_000,
      unlockPeriod: 2_592_000,
      cliffDuration: 7_776_000,
      vestingTotalAmount: 1_000_000_000_000_000n,
      vestingSenderAddress: "0:27feb94b964fccc1d7c772ca4b7e99778846900b55993c50c3eca49b5cd6d727",
      ownerAddress: "0:27feb94b964fccc1d7c772ca4b7e99778846900b55993c50c3eca49b5cd6d727",
    })
  })

  test("releases accumulated periods together when the cliff ends", () => {
    const data = parseVestingData(vestingResponse)
    const beforeCliff = buildVestingSchedule(data, data.vestingStartTime + data.cliffDuration - 1)
    const atCliff = buildVestingSchedule(data, data.vestingStartTime + data.cliffDuration)
    const completed = buildVestingSchedule(data, data.vestingStartTime + data.vestingTotalDuration)

    expect(beforeCliff.unlockedPeriods).toBe(0)
    expect(beforeCliff.periods.slice(0, 3).every(period => period.status === "next")).toBe(true)
    expect(new Set(beforeCliff.periods.slice(0, 3).map(period => period.payoutTime)).size).toBe(1)
    expect(atCliff.unlockedPeriods).toBe(3)
    expect(atCliff.unlockedAmount).toBe(250_000_000_000_000n)
    expect(completed.unlockedPeriods).toBe(12)
    expect(completed.lockedAmount).toBe(0n)
    expect(completed.nextPayoutTime).toBeUndefined()
    expect(completed.periods.reduce((total, period) => total + period.amount, 0n)).toBe(
      data.vestingTotalAmount,
    )
  })

  test("rejects malformed get-method data and invalid schedule parameters", () => {
    expect(() =>
      parseVestingData({...vestingResponse, stack: vestingResponse.stack.slice(0, 7)}),
    ).toThrow("7 stack values instead of 8")
    expect(() =>
      buildVestingSchedule(
        {
          ...parseVestingData(vestingResponse),
          cliffDuration: 1,
        },
        0,
      ),
    ).toThrow("invalid unlock schedule")
  })
})
