// Optional, test-only reference probe. No Node dependency in Go builds or tests.
// Usage: node bool-enum-reference.cjs /path/to/explorer-core/node_modules
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const base = path.resolve(process.argv[2]);
const core = require(path.join(base, '@ton/core'));
const { DynamicCtx } = require(path.join(base, '@ton/tolk-abi-to-typescript/dist/dynamic-ctx'));
const { dynamicPack, dynamicUnpack } = require(path.join(base, '@ton/tolk-abi-to-typescript/dist/dynamic-serialization'));
const { callGetMethodDynamic } = require(path.join(base, '@ton/tolk-abi-to-typescript/dist/dynamic-get-methods'));
const abi = JSON.parse(fs.readFileSync(path.join(__dirname, 'bool-enum.abi.json'), 'utf8'));
const ctx = new DynamicCtx(abi);

async function main() {
    const cells = [];
    for (const value of [false, true]) {
        const cell = core.beginCell().storeBit(value).endCell();
        for (const index of [1, 10]) {
            const slice = cell.beginParse();
            assert.equal(dynamicUnpack(ctx, 'enum', index, slice), value);
            slice.endParse();
        }
        assert.throws(() => dynamicPack(ctx, 'enum', 1, value, core.beginCell()), /number/);
        await assert.rejects(() => callGetMethodDynamic({}, ctx, 'direct', [value]), /number/);
        cells.push({ value, bits: cell.bits.toString(), boc: cell.toBoc().toString('base64') });
    }
    const stack = [];
    for (const value of [0n, -1n, 1n, 42n]) {
        const provider = {
            async get(name, args) {
                assert.equal(name, 'direct');
                assert.deepEqual(args, [{ type: 'int', value }]);
                return { stack: new core.TupleReader(args) };
            },
        };
        assert.equal(await callGetMethodDynamic(provider, ctx, 'direct', [value]), value);
        stack.push(value.toString());
    }
    console.log(JSON.stringify({ cells, numericEnumStack: stack, booleanEnumStackInput: 'rejected' }, null, 2));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
