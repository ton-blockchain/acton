import process from "node:process";

import { compileFunc } from "./languages/func.mjs";
import { compileTact } from "./languages/tact.mjs";
import { compileTolk } from "./languages/tolk.mjs";
import { readStdin, validateInput, writeOutput } from "./languages/common.mjs";

try {
  const input = JSON.parse(await readStdin(process.stdin));
  validateInput(input);
  writeOutput(process.stdout, await compile(input));
} catch (error) {
  writeOutput(process.stdout, {
    status: "compile_error",
    error: error instanceof Error ? error.message : String(error),
  });
}

async function compile(input) {
  switch (input.language) {
    case "func":
      return compileFunc(input);
    case "tact":
      return compileTact(input);
    case "tolk":
      return compileTolk(input);
    default:
      throw new Error(`unsupported language: ${input.language}`);
  }
}
