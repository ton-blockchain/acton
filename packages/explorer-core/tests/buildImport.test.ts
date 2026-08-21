import {describe, expect, test} from "bun:test"

import {
  buildAbiImportPlan,
  buildSourceImportPlan,
  type AbiImportFile,
} from "../src/components/buildImport"

const HASH_A = "1BFC588273D9DE92326658D6ADFC7762B322DD850E3EA84FC3D0B4AA04E3AAAA"
const HASH_B = "2c0e51710a1bc02b0fa1a2a2f162b0a3df6a32d1c2c1b0e51710a1bc02b0bbbb"

function compilerAbi(name: string): Record<string, unknown> {
  return {
    abi_schema_version: 1,
    contract_name: name,
    get_methods: [],
    incoming_messages: [],
    incoming_external: [],
    outgoing_messages: [],
    emitted_events: [],
    declarations: [],
    thrown_errors: [],
  }
}

function abiFile(path: string, name: string): AbiImportFile {
  return {path, text: JSON.stringify(compilerAbi(name))}
}

function codeFile(path: string, hash: string): AbiImportFile {
  return {path, text: JSON.stringify({code_boc64: "te6cc...", hash})}
}

describe("buildAbiImportPlan", () => {
  test("pairs build/abi/<Name>.json with build/<Name>.json by basename", () => {
    const plan = buildAbiImportPlan([
      abiFile("build/abi/WalletSpam.json", "WalletSpam"),
      abiFile("build/abi/JettonWallet.json", "JettonWallet"),
      codeFile("build/WalletSpam.json", HASH_A),
      codeFile("build/JettonWallet.json", HASH_B),
    ])

    expect(plan.warnings).toEqual([])
    expect(plan.registeredNames.toSorted()).toEqual(["JettonWallet", "WalletSpam"])
    const walletSpam = plan.registrations.find(
      entry => entry.abi.compiler_abi.contract_name === "WalletSpam",
    )
    expect(walletSpam?.abi.code_hashes).toEqual([HASH_A.toLowerCase()])
    expect(walletSpam?.abi.display_name).toBe("WalletSpam")
  })

  test("warns and skips an ABI with no matching code artifact", () => {
    const plan = buildAbiImportPlan([abiFile("build/abi/Fat.json", "Fat")])

    expect(plan.registrations).toEqual([])
    expect(plan.warnings).toHaveLength(1)
    expect(plan.warnings[0]).toContain("Fat")
  })

  test("ignores files inside cache/, logs/ and sessions/", () => {
    const plan = buildAbiImportPlan([
      abiFile("build/abi/Fat.json", "Fat"),
      codeFile("build/Fat.json", HASH_A),
      codeFile("build/cache/deadbeef.json", HASH_B),
      {path: "build/logs/run.json", text: JSON.stringify({hash: HASH_B})},
      {path: "build/sessions/s1.json", text: JSON.stringify(compilerAbi("Stale"))},
    ])

    expect(plan.registeredNames).toEqual(["Fat"])
    expect(plan.registrations[0]?.abi.code_hashes).toEqual([HASH_A.toLowerCase()])
  })

  test("prefers the code artifact next to the abi/ directory over same-named files elsewhere", () => {
    const plan = buildAbiImportPlan([
      abiFile("project/build/abi/WalletSpam.json", "WalletSpam"),
      codeFile("project/build/WalletSpam.json", HASH_A),
      codeFile("project/WalletSpam.json", HASH_B),
    ])

    expect(plan.registrations[0]?.abi.code_hashes).toEqual([HASH_A.toLowerCase()])
  })

  test("collects all distinct hashes when same-named artifacts differ", () => {
    const plan = buildAbiImportPlan([
      abiFile("abi/WalletSpam.json", "WalletSpam"),
      codeFile("v1/WalletSpam.json", HASH_A),
      codeFile("v2/WalletSpam.json", HASH_B),
    ])

    expect(plan.registrations[0]?.abi.code_hashes?.toSorted()).toEqual(
      [HASH_A.toLowerCase(), HASH_B.toLowerCase()].toSorted(),
    )
  })

  test("falls back to matching by contract_name when basenames differ", () => {
    const plan = buildAbiImportPlan([
      abiFile("build/abi/wallet_spam_abi.json", "WalletSpam"),
      codeFile("build/WalletSpam.json", HASH_A),
    ])

    expect(plan.registeredNames).toEqual(["WalletSpam"])
    expect(plan.registrations[0]?.abi.code_hashes).toEqual([HASH_A.toLowerCase()])
  })

  test("registers extended ABIs with inline code hashes directly", () => {
    const plan = buildAbiImportPlan([
      {
        path: "exported.json",
        text: JSON.stringify({
          compiler_abi: compilerAbi("Exported"),
          display_name: "My Export",
          code_hashes: [HASH_A],
          links: [],
        }),
      },
    ])

    expect(plan.registeredNames).toEqual(["My Export"])
    expect(plan.registrations[0]?.abi.code_hashes).toEqual([HASH_A.toLowerCase()])
  })

  test("skips unparsable, non-json and unrelated files silently", () => {
    const plan = buildAbiImportPlan([
      {path: "build/abi/Broken.json", text: "{not json"},
      {path: "build/notes.txt", text: JSON.stringify(compilerAbi("Fat"))},
      {path: "build/random.json", text: JSON.stringify({foo: "bar"})},
      abiFile("build/abi/Fat.json", "Fat"),
      codeFile("build/Fat.json", HASH_A),
    ])

    expect(plan.registeredNames).toEqual(["Fat"])
    expect(plan.warnings).toEqual([])
  })

  test("normalizes base64 code hashes to hex", () => {
    const base64Hash = Buffer.from(HASH_A, "hex").toString("base64")
    const plan = buildAbiImportPlan([
      abiFile("build/abi/Fat.json", "Fat"),
      codeFile("build/Fat.json", base64Hash),
    ])

    expect(plan.registrations[0]?.abi.code_hashes).toEqual([HASH_A.toLowerCase()])
  })

  test("dedupes registrations that resolve to the same code hash", () => {
    const plan = buildAbiImportPlan([
      abiFile("a/abi/Fat.json", "Fat"),
      codeFile("a/Fat.json", HASH_A),
      {
        path: "exported.json",
        text: JSON.stringify({compiler_abi: compilerAbi("Fat"), code_hashes: [HASH_A]}),
      },
    ])

    expect(plan.registrations).toHaveLength(1)
  })

  test("merges alias hashes instead of dropping overlapping registrations", () => {
    const plan = buildAbiImportPlan([
      {
        path: "first.json",
        text: JSON.stringify({compiler_abi: compilerAbi("Fat"), code_hashes: [HASH_A]}),
      },
      {
        path: "second.json",
        text: JSON.stringify({compiler_abi: compilerAbi("Fat"), code_hashes: [HASH_A, HASH_B]}),
      },
    ])

    expect(plan.registrations).toHaveLength(1)
    expect(plan.registrations[0]?.abi.code_hashes.map(h => h.toLowerCase()).toSorted()).toEqual(
      [HASH_A.toLowerCase(), HASH_B.toLowerCase()].toSorted(),
    )
  })

  test("dedupes on any shared hash, not only the first one", () => {
    const plan = buildAbiImportPlan([
      {
        path: "first.json",
        text: JSON.stringify({compiler_abi: compilerAbi("Fat"), code_hashes: [HASH_A, HASH_B]}),
      },
      {
        path: "second.json",
        text: JSON.stringify({compiler_abi: compilerAbi("Fat"), code_hashes: [HASH_B]}),
      },
    ])

    expect(plan.registrations).toHaveLength(1)
  })

  test("keeps same-named ABIs from separate build trees", () => {
    const plan = buildAbiImportPlan([
      abiFile("project-a/build/abi/Wallet.json", "Wallet"),
      codeFile("project-a/build/Wallet.json", HASH_A),
      abiFile("project-b/build/abi/Wallet.json", "Wallet"),
      codeFile("project-b/build/Wallet.json", HASH_B),
    ])

    expect(plan.registrations).toHaveLength(2)
    expect(plan.registrations.flatMap(entry => [...entry.abi.code_hashes]).toSorted()).toEqual(
      [HASH_A.toLowerCase(), HASH_B.toLowerCase()].toSorted(),
    )
  })
})

function sourceBundle(entrypoint: string): Record<string, unknown> {
  return {
    source_bundle_hash: "ab".repeat(32),
    verified_at: 0,
    storage_revision: "local",
    entrypoint,
    compiler: {language: "tolk", version: "1.4.2", params: {}},
    files: [
      {
        path: entrypoint,
        content_hash: "cd".repeat(32),
        include_in_command: true,
        is_stdlib: false,
        has_include_directives: null,
        content: "fun onInternalMessage() {}",
      },
    ],
  }
}

function sourceArtifactFile(path: string, hash: string, entrypoint: string): AbiImportFile {
  return {
    path,
    text: JSON.stringify({code_hash: hash, verified: true, bundle: sourceBundle(entrypoint)}),
  }
}

describe("buildSourceImportPlan", () => {
  test("finds source artifacts anywhere in a dropped project tree", () => {
    const plan = buildSourceImportPlan([
      {path: "project/Acton.toml.json", text: "not json"},
      {path: "project/tps_shards.json", text: JSON.stringify({shards: []})},
      abiFile("project/build/abi/WalletSpam.json", "WalletSpam"),
      codeFile("project/build/WalletSpam.json", HASH_A),
      sourceArtifactFile(
        "project/build/sources/WalletSpam.source.json",
        HASH_A,
        "contracts/WalletSpam.tolk",
      ),
      sourceArtifactFile("project/build/sources/Fat.source.json", HASH_B, "contracts/Fat.tolk"),
    ])

    expect(plan.warnings).toEqual([])
    expect(plan.registeredNames.toSorted()).toEqual([
      "contracts/Fat.tolk",
      "contracts/WalletSpam.tolk",
    ])
    expect(plan.registrations.map(entry => entry.codeHash).toSorted()).toEqual(
      [HASH_A.toLowerCase(), HASH_B.toLowerCase()].toSorted(),
    )
  })

  test("accepts the legacy bundles-array artifact shape", () => {
    const plan = buildSourceImportPlan([
      {
        path: "build/sources/WalletSpam.source.json",
        text: JSON.stringify({
          code_hash: HASH_A,
          verified: true,
          bundles: [sourceBundle("contracts/WalletSpam.tolk")],
        }),
      },
    ])

    expect(plan.registrations).toHaveLength(1)
    expect(plan.registrations[0]?.codeHash).toBe(HASH_A.toLowerCase())
    expect(plan.registrations[0]?.source.bundle?.entrypoint).toBe("contracts/WalletSpam.tolk")
  })

  test("dedupes artifacts with the same code hash", () => {
    const plan = buildSourceImportPlan([
      sourceArtifactFile("a/WalletSpam.source.json", HASH_A, "contracts/WalletSpam.tolk"),
      sourceArtifactFile("b/WalletSpam.source.json", HASH_A, "contracts/WalletSpam.tolk"),
    ])

    expect(plan.registrations).toHaveLength(1)
  })

  test("warns with a how-to when no artifacts are present", () => {
    const plan = buildSourceImportPlan([
      abiFile("build/abi/Fat.json", "Fat"),
      codeFile("build/Fat.json", HASH_A),
    ])

    expect(plan.registrations).toEqual([])
    expect(plan.warnings[0]).toContain("acton build --output-sources")
  })

  test("ignores artifacts inside skipped directories", () => {
    const plan = buildSourceImportPlan([
      sourceArtifactFile("build/cache/WalletSpam.source.json", HASH_A, "contracts/WalletSpam.tolk"),
    ])

    expect(plan.registrations).toEqual([])
  })

  test("finds artifacts under .studio/ while other hidden dirs stay skipped", () => {
    const plan = buildSourceImportPlan([
      sourceArtifactFile(
        "project/.studio/sources/Simple.source.json",
        HASH_A,
        "contracts/Simple.tolk",
      ),
      sourceArtifactFile("project/.git/objects/Fake.source.json", HASH_B, "contracts/Fake.tolk"),
    ])

    expect(plan.registrations).toHaveLength(1)
    expect(plan.registrations[0]?.codeHash).toBe(HASH_A.toLowerCase())
  })
})
