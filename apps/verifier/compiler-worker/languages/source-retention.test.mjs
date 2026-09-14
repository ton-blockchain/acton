import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { compileFunc } from "./func.mjs";
import { compileTact } from "./tact.mjs";
import { compileTolk } from "./tolk.mjs";

async function fixture(name) {
  const url = new URL(`../fixtures/${name}`, import.meta.url);
  return JSON.parse(await readFile(url, "utf8"));
}

test("Tolk reports the entrypoint and imported sources", async () => {
  const input = await fixture("valid-import-mapping.json");
  input.sources.push({
    path: "unused.tolk",
    content: "fun unused() {}\n",
  });

  const result = await compileTolk(input);

  assert.equal(result.status, "ok");
  assert.deepEqual(result.used_source_paths, ["contracts/lib.tolk", "main.tolk"]);
});

test("FunC reports targets and included sources", async () => {
  const input = await fixture("valid-func.json");
  input.sources[0].content =
    '#include "lib.fc";\n() recv_internal(int my_balance, int msg_value, cell in_msg_full, slice in_msg_body) impure {\n}\n';
  input.sources.push(
    {
      path: "lib.fc",
      content: "int helper() { return 1; }\n",
    },
    {
      path: "unused.fc",
      content: "int unused() { return 2; }\n",
    },
  );

  const result = await compileFunc(input);

  assert.equal(result.status, "ok");
  assert.deepEqual(result.used_source_paths, ["lib.fc", "main.fc"]);
});

test("Tact reports only the selected package as an uploaded input", async () => {
  const input = await fixture("valid-tact.json");
  input.sources.push({
    path: "unused.tact",
    content: "contract Unused {}\n",
  });

  const result = await compileTact(input);

  assert.equal(result.status, "ok");
  assert.deepEqual(result.used_source_paths, ["contract.pkg"]);
});
