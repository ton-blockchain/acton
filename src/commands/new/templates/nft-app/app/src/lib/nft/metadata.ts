import { beginCell, Cell, Dictionary } from '@ton/core';
import { sha256_sync } from '@ton/crypto';

const ONCHAIN_CONTENT_PREFIX = 0x00;
const SNAKE_PREFIX = 0x00;
const OFFCHAIN_CONTENT_PREFIX = 0x01;

function snakeStringCell(value: string): Cell {
  return beginCell()
    .storeUint(SNAKE_PREFIX, 8)
    .storeStringTail(value)
    .endCell();
}

function sha256BigInt(key: string): bigint {
  const hex = sha256_sync(key).toString('hex');
  return BigInt('0x' + hex);
}

export interface OnchainMetadataFields {
  name?: string;
  description?: string;
  image?: string;
}

export interface CollectionMetadata {
  name?: string;
  description?: string;
  image?: string;
  uri?: string;
}

/**
 * On-chain TEP-64 metadata: prefix 0x00 + dict sha256(key) => snake(value).
 * Matches `buildOnchainMetadata` in contracts/scripts/utils/common.tolk.
 */
export function buildOnchainMetadataCell(fields: OnchainMetadataFields): Cell {
  const dict = Dictionary.empty(
    Dictionary.Keys.BigUint(256),
    Dictionary.Values.Cell(),
  );
  if (fields.name !== undefined)
    dict.set(sha256BigInt('name'), snakeStringCell(fields.name));
  if (fields.description !== undefined)
    dict.set(sha256BigInt('description'), snakeStringCell(fields.description));
  if (fields.image !== undefined)
    dict.set(sha256BigInt('image'), snakeStringCell(fields.image));
  return beginCell()
    .storeUint(ONCHAIN_CONTENT_PREFIX, 8)
    .storeDict(dict)
    .endCell();
}

/** Off-chain TEP-64 metadata: prefix 0x01 + URI. */
export function buildOffchainMetadataCell(uri: string): Cell {
  return beginCell()
    .storeUint(OFFCHAIN_CONTENT_PREFIX, 8)
    .storeStringTail(uri)
    .endCell();
}

function decodeSnakeString(cell: Cell): string {
  const s = cell.beginParse();
  if (s.remainingBits >= 8) {
    const prefix = s.preloadUint(8);
    if (prefix === 0x00) s.loadUint(8);
  }
  return s.loadStringTail();
}

export function decodeCollectionMetadata(
  metadataCell: Cell,
): CollectionMetadata {
  try {
    const s = metadataCell.beginParse();
    if (s.remainingBits < 8) return {};
    const prefix = s.loadUint(8);
    if (prefix === OFFCHAIN_CONTENT_PREFIX) {
      return { uri: s.loadStringTail() };
    }
    if (prefix === ONCHAIN_CONTENT_PREFIX) {
      const dict = s.loadDict(
        Dictionary.Keys.BigUint(256),
        Dictionary.Values.Cell(),
      );
      const out: CollectionMetadata = {};
      const nameCell = dict.get(sha256BigInt('name'));
      const descCell = dict.get(sha256BigInt('description'));
      const imageCell = dict.get(sha256BigInt('image'));
      if (nameCell) out.name = decodeSnakeString(nameCell);
      if (descCell) out.description = decodeSnakeString(descCell);
      if (imageCell) out.image = decodeSnakeString(imageCell);
      return out;
    }
    return {};
  } catch {
    return {};
  }
}
