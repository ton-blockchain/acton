import {afterEach, describe, expect, spyOn, test} from "bun:test"
import {Cell, loadShardAccount} from "@ton/core"

import {TonClient} from "../src/api/client"
import {
  ConfigVotingReader,
  configVoteRegistrations,
  configVoteTally,
  parseConfigVotingState,
  type ConfigVotingSnapshot,
} from "../src/api/configVoting"
import fixture from "./fixtures/mainnet-voting-89031014.json"
import testnetRegistrations from "./fixtures/testnet-voting-registrations.json"
import type {V3Transaction} from "../src/api/types"

const configAddress = `-1:${"55".repeat(32)}`
const proposalHash = "e5e148027499276e65c48749129ca014bb4e53279b1ac6314d124f4e30166076"
const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

function readFixtureState(which: "before" | "after") {
  const state = fixture[which]
  const account = loadShardAccount(Cell.fromBase64(state.boc).beginParse()).account
  if (account?.storage.state.type !== "active" || !account.storage.state.state.data) {
    throw new Error("Fixture has no active config account")
  }

  return account.storage.state.state.data
}

function readSnapshot(which: "before" | "after") {
  return parseConfigVotingState(
    readFixtureState(which),
    configAddress,
    fixture[which].seqno,
    fixture[which].time,
  )
}

function summarize(snapshot: ConfigVotingSnapshot) {
  return snapshot.proposals.map(vote => {
    const tally = configVoteTally(vote, snapshot)
    return {
      hash: vote.hash,
      parameter: vote.parameterId,
      critical: vote.critical,
      expires: vote.expires,
      wins: vote.state?.wins,
      losses: vote.state?.losses,
      rounds: vote.state?.rounds_remaining,
      voters: vote.state?.voters.size,
      percent: tally?.percent,
      neededValidators: tally?.neededValidators,
      won: tally?.won,
      currentSet: tally?.currentSet,
    }
  })
}

function createClient(network = "mainnet") {
  const base = `https://studio.test/api/v1/environments/${network}/rpc`
  return new TonClient({
    v2BaseUrl: `${base}/api/v2`,
    v3BaseUrl: `${base}/api/v3`,
    addressNameBaseUrl: base,
    localnetControlEnabled: false,
  })
}

function mockArchive(withReceipt = true) {
  const requests: string[] = []

  spyOn(globalThis, "fetch").mockImplementation(async input => {
    const url = new URL(String(input))
    requests.push(`${url.pathname}${url.search}`)

    if (url.pathname.endsWith("/getConfigAll")) {
      return Response.json({
        ok: true,
        result: {config: {bytes: readFixtureState("after").refs[0].toBoc().toString("base64")}},
      })
    }

    if (url.pathname.endsWith("/blocks")) {
      const seqno = Number(url.searchParams.get("seqno") ?? fixture.after.seqno)
      return Response.json({
        blocks: [
          {
            seqno,
            gen_utime: seqno === fixture.before.seqno ? fixture.before.time : fixture.after.time,
          },
        ],
      })
    }

    if (url.pathname.endsWith("/getShardAccountCell")) {
      const seqno = Number(url.searchParams.get("seqno"))
      if (seqno !== fixture.before.seqno && seqno !== fixture.after.seqno)
        throw new Error(`Unexpected archive block ${seqno}`)
      return Response.json({
        ok: true,
        result: {bytes: seqno === fixture.before.seqno ? fixture.before.boc : fixture.after.boc},
      })
    }

    if (url.pathname.endsWith("/transactionsByMessage")) {
      const registration = url.searchParams.get("opcode") === "0x6e565052"
      const transactions = registration
        ? [fixture.registration]
        : withReceipt
          ? [fixture.acceptance]
          : []
      return Response.json({transactions})
    }

    throw new Error(`Unexpected request ${url}`)
  })

  return requests
}

describe("configuration voting", () => {
  test("reads real Mainnet storage including timestamp-bearing voter dictionary leaves", () => {
    // Captured from TON Center on 2026-09-07, immediately before parameter 30 was accepted.
    expect(summarize(readSnapshot("before"))).toMatchSnapshot()
    expect(summarize(readSnapshot("after"))).toMatchSnapshot()
  })

  test("loads history and verifies an accepted proposal through the selected network client", async () => {
    const requests = mockArchive()
    const reader = new ConfigVotingReader(createClient())
    const signal = new AbortController().signal
    const current = await reader.current(signal)
    const history = await reader.history(current.address, 0, signal)
    const result = await reader.result(history.proposals[0], current, signal, () => {})
    const tally = configVoteTally(result.vote, result.snapshot)

    expect({
      registered: history.proposals.map(vote => ({hash: vote.hash, parameter: vote.parameterId})),
      outcome: result.outcome,
      seqno: result.seqno,
      time: result.time,
      wins: result.vote.state?.wins,
      losses: result.vote.state?.losses,
      voters: result.vote.state?.voters.size,
      percent: tally?.percent,
      won: tally?.won,
      finalVoteRecorded: result.finalVoteRecorded,
      removed: result.removed,
      requests,
    }).toMatchSnapshot()

    // Adding the confirming voter must not mutate the cached pre-completion snapshot.
    expect(
      summarize(await reader.at(configAddress, fixture.before.seqno, signal)),
    ).toMatchSnapshot()
  })

  test("resolves a direct proposal link past a full history page before verifying its result", async () => {
    const requests = mockArchive()
    const archiveFetch = globalThis.fetch

    globalThis.fetch = Object.assign(async (input: Parameters<typeof fetch>[0]) => {
      const url = new URL(String(input))

      if (
        url.pathname.endsWith("/transactionsByMessage") &&
        url.searchParams.get("opcode") === "0x6e565052" &&
        url.searchParams.get("offset") === "0"
      ) {
        requests.push(`${url.pathname}${url.search}`)

        // A full indexer page may contain no successful registrations for this
        // config contract. The direct lookup must still follow its cursor.
        return Response.json({
          transactions: Array.from({length: 10}, () => ({
            ...fixture.registration,
            description: {...fixture.registration.description, aborted: true},
          })),
        })
      }

      return archiveFetch(input)
    }, archiveFetch)

    const reader = new ConfigVotingReader(createClient("testnet"))
    const signal = new AbortController().signal
    const progress: string[] = []
    const {vote, snapshot} = await reader.proposal(proposalHash.toUpperCase(), signal, message => {
      progress.push(message)
    })
    const result = await reader.result(vote, snapshot, signal, () => {})

    expect({hash: vote.hash, outcome: result.outcome, progress, requests}).toMatchSnapshot()
  })

  test("opens a stored proposal without reading history and stops invalid or cancelled lookups", async () => {
    const requests = mockArchive()
    const reader = new ConfigVotingReader(createClient())
    const current = spyOn(reader, "current").mockResolvedValue(readSnapshot("before"))
    const signal = new AbortController().signal
    const {vote} = await reader.proposal(proposalHash, signal, () => {})
    current.mockRestore()

    const failures: string[] = []
    const cancelled = new AbortController()
    cancelled.abort(new Error("Navigation cancelled"))

    for (const [hash, requestSignal] of [
      ["invalid", signal],
      [proposalHash, cancelled.signal],
      ["0".repeat(64), signal],
    ] as const) {
      try {
        await reader.proposal(hash, requestSignal, () => {})
      } catch (reason) {
        failures.push((reason as Error).message)
      }
    }

    expect({
      hash: vote.hash,
      hasVotingState: Boolean(vote.state),
      failures,
      requests,
    }).toMatchSnapshot()
  })

  test("keeps an unproven disappearance distinct from acceptance or rejection", async () => {
    mockArchive(false)
    const reader = new ConfigVotingReader(createClient())
    const before = readSnapshot("before")
    const after = {...readSnapshot("after"), config: before.config}
    const proposal = before.proposals.find(proposal => proposal.hash === proposalHash)
    if (!proposal?.state) throw new Error("Fixture is missing proposal 30")

    const vote = {...proposal, registrationSeqno: before.seqno}
    const result = await reader.result(vote, after, new AbortController().signal, () => {})

    expect({
      outcome: result.outcome,
      finalVoteRecorded: result.finalVoteRecorded,
      wins: result.vote.state?.wins,
    }).toMatchSnapshot()
  })

  test("finds the exact archive transition when acceptance has no outgoing receipt", async () => {
    mockArchive(false)
    const reader = new ConfigVotingReader(createClient())
    const before = readSnapshot("before")
    const after = readSnapshot("after")
    const proposal = before.proposals.find(proposal => proposal.hash === proposalHash)
    if (!proposal?.state) throw new Error("Fixture is missing proposal 30")

    // Expand the interval while retaining the real boundary states. This exercises
    // both halves of the archive search and the external-vote acceptance path.
    const checkedBlocks: number[] = []
    spyOn(reader, "at").mockImplementation(async (_address, seqno) => {
      checkedBlocks.push(seqno)
      return {...(seqno < after.seqno ? before : after), seqno}
    })
    const result = await reader.result(
      {...proposal, registrationSeqno: before.seqno - 12},
      {...after, seqno: after.seqno + 15},
      new AbortController().signal,
      () => {},
    )

    expect({
      outcome: result.outcome,
      seqno: result.seqno,
      finalVoteRecorded: result.finalVoteRecorded,
      checkedBlocks,
    }).toMatchSnapshot()
  })

  test("a passing receipt does not imply that a conditional value was applied", async () => {
    mockArchive()
    const reader = new ConfigVotingReader(createClient())
    const before = readSnapshot("before")
    const after = {...readSnapshot("after"), config: before.config}
    const proposal = before.proposals.find(proposal => proposal.hash === proposalHash)
    if (!proposal?.state) throw new Error("Fixture is missing proposal 30")

    spyOn(reader, "at").mockImplementation(async (_address, seqno) =>
      seqno === before.seqno ? before : after,
    )
    const result = await reader.result(
      {...proposal, registrationSeqno: before.seqno},
      after,
      new AbortController().signal,
      () => {},
    )

    expect({
      outcome: result.outcome,
      wins: result.vote.state?.wins,
      removed: result.removed,
    }).toMatchSnapshot()
  })

  test("recognizes expiry before lazy contract cleanup and does not query the archive", async () => {
    const requests = mockArchive()
    const reader = new ConfigVotingReader(createClient())
    const snapshot = readSnapshot("before")
    const vote = snapshot.proposals.find(proposal => proposal.hash === proposalHash)
    if (!vote?.state) throw new Error("Fixture is missing proposal 30")

    const result = await reader.result(
      vote,
      {...snapshot, time: vote.state.expires},
      new AbortController().signal,
      () => {},
    )

    expect({outcome: result.outcome, removed: result.removed, requests}).toMatchSnapshot()
  })

  test("requires strictly more than 75 percent and never applies weights from a different set", () => {
    const snapshot = readSnapshot("before")
    const vote = snapshot.proposals.find(proposal => proposal.hash === proposalHash)
    if (!vote?.state) throw new Error("Fixture is missing proposal 30")

    const atThreshold = {...vote, state: {...vote.state, remaining_weight: 0n}}
    const won = {...vote, state: {...vote.state, remaining_weight: -1n}}
    const unknownSet = {...vote, state: {...vote.state, validator_set_id: 0n}}

    expect({
      atThreshold: configVoteTally(atThreshold, snapshot)?.won,
      aboveThreshold: configVoteTally(won, snapshot)?.won,
      unknownSet: configVoteTally(unknownSet, snapshot),
    }).toMatchSnapshot()
  })

  test("filters failed registrations and reads Testnet history from its own endpoint", async () => {
    const registration = fixture.registration as V3Transaction
    const candidates = [
      registration,
      {...registration, account: `-1:${"22".repeat(32)}`},
      {...registration, description: {...registration.description, aborted: true}},
      {...registration, out_msgs: []},
    ]
    expect(
      configVoteRegistrations(candidates, configAddress).map(vote => vote.hash),
    ).toMatchSnapshot()

    const requests: string[] = []
    spyOn(globalThis, "fetch").mockImplementation(async input => {
      requests.push(String(input))
      return Response.json(testnetRegistrations)
    })
    const reader = new ConfigVotingReader(createClient("testnet"))
    const history = await reader.history(configAddress, 0, new AbortController().signal)

    expect({
      requests,
      proposals: history.proposals.map(vote => ({hash: vote.hash, parameter: vote.parameterId})),
    }).toMatchSnapshot()
  })

  test("stops archive work when the environment or selected proposal changes", async () => {
    const requests = mockArchive(false)
    const reader = new ConfigVotingReader(createClient())
    const controller = new AbortController()
    controller.abort()

    await expect(
      reader.at(configAddress, fixture.before.seqno, controller.signal),
    ).rejects.toThrow()
    expect(requests).toMatchSnapshot()
  })
})
