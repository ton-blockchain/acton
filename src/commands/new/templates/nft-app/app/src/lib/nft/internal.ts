import { beginCell, Cell, storeStateInit } from '@ton/core';

const TX_TTL_SECONDS = 5 * 60;

export function txValidUntil() {
  return Math.floor(Date.now() / 1000) + TX_TTL_SECONDS;
}

export function toB64(cell: Cell) {
  return cell.toBoc().toString('base64');
}

export function encodeStateInit(contract: {
  init?: { code: Cell; data: Cell };
}): string {
  if (!contract.init) throw new Error('Contract init missing');
  return beginCell()
    .store(storeStateInit(contract.init))
    .endCell()
    .toBoc()
    .toString('base64');
}
