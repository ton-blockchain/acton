import assert from "node:assert/strict";
import test from "node:test";
import { getMethodId } from "@ton/core";

import { generatedTolkAbiSources } from "./tact-abi.mjs";

test("converts a Tact ABI to Tolk types and compiler ABI", async () => {
  const tactAbi = {
    name: "StableMaster",
    types: [
      {
        name: "Balance",
        header: null,
        fields: [
          {
            name: "amount",
            type: {
              kind: "simple",
              type: "uint",
              optional: false,
              format: "coins",
            },
          },
        ],
      },
      {
        name: "Payload",
        header: 0x12345678,
        fields: [
          {
            name: "signature",
            type: {
              kind: "simple",
              type: "fixed-bytes",
              optional: false,
              format: 64,
            },
          },
          {
            name: "balance",
            type: {
              kind: "simple",
              type: "Balance",
              optional: true,
              format: "ref",
            },
          },
          {
            name: "tail",
            type: {
              kind: "simple",
              type: "slice",
              optional: false,
              format: "remainder",
            },
          },
        ],
      },
      {
        name: "UnsupportedPayload",
        header: 0x87654321,
        fields: [
          {
            name: "unsupported",
            type: { kind: "tuple", items: [] },
          },
        ],
      },
      {
        name: "StableMaster$Data",
        header: null,
        fields: [
          {
            name: "balances",
            type: {
              kind: "dict",
              key: "uint",
              keyFormat: 256,
              value: "Balance",
              valueFormat: "ref",
            },
          },
          {
            name: "legacyBalances",
            type: {
              kind: "dict",
              key: "uint",
              keyFormat: 256,
              value: "Balance",
            },
          },
          {
            name: "owner",
            type: { kind: "simple", type: "address", optional: true },
          },
          {
            name: "sequenceNumber",
            type: {
              kind: "simple",
              type: "int",
              optional: false,
              format: 257,
            },
          },
        ],
      },
    ],
    receivers: [
      { receiver: "internal", message: { kind: "typed", type: "Payload" } },
      {
        receiver: "internal",
        message: { kind: "typed", type: "UnsupportedPayload" },
      },
      { receiver: "external", message: { kind: "typed", type: "Payload" } },
    ],
    getters: [
      {
        name: "getBalance",
        methodId: 777,
        arguments: [
          {
            name: "owner",
            type: { kind: "simple", type: "address", optional: false },
          },
        ],
        returnType: { kind: "simple", type: "Balance", optional: false },
      },
      {
        name: "seqno",
        arguments: [],
        returnType: {
          kind: "simple",
          type: "int",
          optional: false,
          format: 257,
        },
      },
      {
        name: "address",
        arguments: [],
        returnType: {
          kind: "simple",
          type: "address",
          optional: false,
        },
      },
      {
        name: "random",
        methodId: 7777,
        arguments: [],
        returnType: {
          kind: "simple",
          type: "int",
          optional: false,
          format: 257,
        },
      },
    ],
    errors: { 401: { message: "Unauthorized sender" } },
  };

  const sources = await generatedTolkAbiSources(
    {
      name: tactAbi.name,
      abi: JSON.stringify(tactAbi),
      compiler: { version: "1.6.13" },
    },
    [
      {
        path: "output/verifier_StableMaster.abi",
        content: JSON.stringify(tactAbi),
      },
    ],
  );

  assert.deepEqual(
    sources.map((source) => source.path),
    ["output/StableMaster.types.tolk", "output/StableMaster.abi.json"],
  );
  assert.match(sources[0].content, /storage: StableMasterData/);
  assert.match(sources[0].content, /signature: bits512/);
  assert.match(sources[0].content, /balance: Cell<StableMasterBalance>\?/);
  assert.match(
    sources[0].content,
    /balances: map<uint256, Cell<StableMasterBalance>>/,
  );
  assert.match(
    sources[0].content,
    /legacyBalances: map<uint256, Cell<StableMasterBalance>>/,
  );
  assert.match(sources[0].content, /tail: RemainingBitsAndRefs/);
  assert.match(sources[0].content, /sequenceNumber: int257/);
  assert.match(
    sources[0].content,
    /\/\/ Tact method ID: 777\nget fun getBalance/,
  );
  assert.match(sources[0].content, /get fun seqno/);
  assert.doesNotMatch(sources[0].content, /tactAbiGetter_/);
  assert.match(
    sources[0].content,
    /\/\/ Tact getter name: address\n\/\/ Tact method ID: 69216\nget fun address_/,
  );
  assert.match(
    sources[0].content,
    /\/\/ Tact getter name: random\n\/\/ Tact method ID: 7777\nget fun random_/,
  );
  assert.doesNotMatch(sources[0].content, /onInternalMessage/);
  assert.doesNotMatch(sources[0].content, /UnsupportedPayload/);

  const abi = JSON.parse(sources[1].content);
  assert.equal(abi.contract_name, "StableMaster");
  assert.equal(abi.compiler_name, "tolk");
  assert.equal(abi.compiler_version, "1.4.2");
  assert.equal(abi.incoming_messages.length, 1);
  assert.equal(abi.incoming_external.length, 1);
  const customGetter = abi.get_methods.find(
    (getter) => getter.name === "getBalance",
  );
  assert.equal(customGetter.tvm_method_id, 777);
  const defaultGetter = abi.get_methods.find(
    (getter) => getter.name === "seqno",
  );
  assert.equal(defaultGetter.tvm_method_id, getMethodId("seqno"));
  const addressGetter = abi.get_methods.find(
    (getter) => getter.name === "address",
  );
  assert.equal(addressGetter.tvm_method_id, getMethodId("address"));
  const randomGetter = abi.get_methods.find(
    (getter) => getter.name === "random",
  );
  assert.equal(randomGetter.tvm_method_id, 7777);
  assert.deepEqual(abi.thrown_errors, [
    {
      kind: "enum_member",
      name: "StableMasterErrors.UnauthorizedSender",
      description: "Unauthorized sender",
      err_code: 401,
    },
  ]);
});

test("includes Tact runtime metadata in direct-init storage", async () => {
  const tactAbi = { name: "AddressStorage", types: [] };
  const sources = await generatedTolkAbiSources(
    {
      name: tactAbi.name,
      abi: tactAbi,
      compiler: { version: "1.3.0" },
      init: {
        args: [],
        prefix: { bits: 1, value: 0 },
        deployment: { kind: "system-cell", system: "system-cell-boc" },
      },
    },
    [
      {
        path: "contracts/address-storage.tact",
        content: `contract AddressStorage {
    owner: Address;
    pendingOwner: Address?;
    content: Cell;
    initialized: Bool;
}`,
      },
    ],
  );

  assert.match(sources[0].content, /storage: AddressStorageData/);
  assert.match(sources[0].content, /tactSystemCell: cell/);
  assert.match(sources[0].content, /tactDeploymentCompleted: bool/);
  assert.match(sources[0].content, /owner: address/);
  assert.match(sources[0].content, /pendingOwner: address\?/);
  assert.doesNotMatch(sources[0].content, /any_address/);

  const abi = JSON.parse(sources[1].content);
  const storage = abi.declarations.find(
    (declaration) => declaration.name === "AddressStorageData",
  );
  assert.deepEqual(
    storage.fields.map((field) => [
      field.name,
      abi.unique_types[field.ty_idx].kind,
    ]),
    [
      ["tactSystemCell", "cell"],
      ["tactDeploymentCompleted", "bool"],
      ["owner", "address"],
      ["pendingOwner", "addressOpt"],
      ["content", "cell"],
      ["initialized", "bool"],
    ],
  );
});

test("uses contract parameters as storage without a deployment bit", async () => {
  const tactAbi = { name: "Parameterized", types: [] };
  const sources = await generatedTolkAbiSources(
    {
      name: tactAbi.name,
      abi: tactAbi,
      compiler: { version: "1.3.0" },
      init: {
        args: [
          {
            name: "owner",
            type: { kind: "simple", type: "address", optional: false },
          },
          {
            name: "pendingOwner",
            type: { kind: "simple", type: "address", optional: true },
          },
        ],
        deployment: { kind: "system-cell", system: null },
      },
    },
    [],
  );

  assert.match(sources[0].content, /storage: ParameterizedData/);
  assert.match(sources[0].content, /owner: address/);
  assert.match(sources[0].content, /pendingOwner: address\?/);
  assert.doesNotMatch(sources[0].content, /tactDeploymentCompleted/);
  assert.doesNotMatch(sources[0].content, /tactSystemCell/);
});

test("splits large Tact storage into continuation cells", async () => {
  const tactAbi = { name: "GramxToken", types: [] };
  const sources = await generatedTolkAbiSources(
    {
      name: tactAbi.name,
      abi: tactAbi,
      compiler: { version: "1.3.0" },
      init: {
        args: [],
        prefix: { bits: 1, value: 0 },
        deployment: { kind: "system-cell", system: "system-cell-boc" },
      },
    },
    [
      {
        path: "contracts/gramx-token.tact",
        content: `contract GramxToken {
    totalSupply: Int as coins;
    owner: Address;
    pendingOwner: Address?;
    content: Cell;
    mintable: Bool;
    fixedSupply: Int as coins;
    initialReceiver: Address;
    initialized: Bool;
}`,
      },
    ],
  );

  assert.match(
    sources[0].content,
    /fixedSupply: coins\n    tactContinuation: Cell<GramxTokenDataContinuation>/,
  );
  assert.match(
    sources[0].content,
    /struct GramxTokenDataContinuation \{\n    initialReceiver: address\n    initialized: bool\n\}/,
  );

  const abi = JSON.parse(sources[1].content);
  const storage = abi.declarations.find(
    (declaration) => declaration.name === "GramxTokenData",
  );
  assert.deepEqual(
    storage.fields.map((field) => field.name),
    [
      "tactSystemCell",
      "tactDeploymentCompleted",
      "totalSupply",
      "owner",
      "pendingOwner",
      "content",
      "mintable",
      "fixedSupply",
      "tactContinuation",
    ],
  );
});

test("splits large Tact messages across multiple continuation cells", async () => {
  const fields = Array.from({ length: 7 }, (_, index) => ({
    name: `value${index + 1}`,
    type: {
      kind: "simple",
      type: "uint",
      optional: false,
      format: 256,
    },
  }));
  const tactAbi = {
    name: "LargeMessages",
    types: [{ name: "HugeMessage", header: 0x12345678, fields }],
    receivers: [
      {
        receiver: "internal",
        message: { kind: "typed", type: "HugeMessage" },
      },
    ],
  };
  const sources = await generatedTolkAbiSources(
    {
      name: tactAbi.name,
      abi: tactAbi,
      compiler: { version: "1.6.13" },
    },
    [],
  );

  assert.match(
    sources[0].content,
    /value3: uint256\n    tactContinuation: Cell<LargeMessagesHugeMessageContinuation>/,
  );
  assert.match(
    sources[0].content,
    /value6: uint256\n    tactContinuation: Cell<LargeMessagesHugeMessageContinuation2>/,
  );
  assert.match(
    sources[0].content,
    /struct LargeMessagesHugeMessageContinuation2 \{\n    value7: uint256\n\}/,
  );

  const abi = JSON.parse(sources[1].content);
  assert.equal(abi.incoming_messages.length, 1);
  assert.ok(
    abi.declarations.some(
      (declaration) =>
        declaration.name === "LargeMessagesHugeMessageContinuation2",
    ),
  );
});

test("uses split sizes for structs embedded into messages", async () => {
  const uint256 = {
    kind: "simple",
    type: "uint",
    optional: false,
    format: 256,
  };
  const tactAbi = {
    name: "NestedMessages",
    types: [
      {
        name: "HugePayload",
        header: null,
        fields: Array.from({ length: 4 }, (_, index) => ({
          name: `part${index + 1}`,
          type: uint256,
        })),
      },
      {
        name: "Envelope",
        header: 0x12345678,
        fields: [
          {
            name: "payload",
            type: {
              kind: "simple",
              type: "HugePayload",
              optional: false,
            },
          },
          { name: "tail", type: uint256 },
        ],
      },
    ],
    receivers: [
      {
        receiver: "internal",
        message: { kind: "typed", type: "Envelope" },
      },
    ],
  };
  const sources = await generatedTolkAbiSources(
    {
      name: tactAbi.name,
      abi: tactAbi,
      compiler: { version: "1.6.13" },
    },
    [],
  );

  assert.match(
    sources[0].content,
    /part3: uint256\n    tactContinuation: Cell<NestedMessagesHugePayloadContinuation>/,
  );
  assert.match(
    sources[0].content,
    /payload: NestedMessagesHugePayload\n    tactContinuation: Cell<NestedMessagesEnvelopeContinuation>/,
  );
  assert.match(
    sources[0].content,
    /struct NestedMessagesEnvelopeContinuation \{\n    tail: uint256\n\}/,
  );
});

test("omits generated ABI when the Tact ABI cannot be converted", async () => {
  const tactAbi = {
    name: "UnsupportedStorage",
    types: [
      {
        name: "UnsupportedStorage$Data",
        fields: [
          {
            name: "raw",
            type: { kind: "simple", type: "slice", optional: false },
          },
        ],
      },
    ],
  };

  const sources = await generatedTolkAbiSources(
    { name: tactAbi.name, abi: JSON.stringify(tactAbi) },
    [],
  );

  assert.equal(sources, undefined);
});
