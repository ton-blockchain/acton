import { beginCell, Cell, Dictionary } from '@ton/core';

const ONCHAIN_CONTENT_PREFIX = 0x00;
const SNAKE_DATA_PREFIX = 0x00;

const sha256Keys: Record<string, Buffer> = {};

async function sha256(key: string): Promise<Buffer> {
  if (!sha256Keys[key]) {
    const data = new TextEncoder().encode(key);
    const hash = await crypto.subtle.digest('SHA-256', data);
    sha256Keys[key] = Buffer.from(hash);
  }
  return sha256Keys[key]!;
}

function makeSnakeCell(data: Buffer): Cell {
  const firstChunkSize = 126;
  const chunkSize = 127;

  if (data.length <= firstChunkSize) {
    return beginCell().storeUint(SNAKE_DATA_PREFIX, 8).storeBuffer(data).endCell();
  }

  const chunks: Buffer[] = [];
  chunks.push(data.subarray(0, firstChunkSize));
  let offset = firstChunkSize;
  while (offset < data.length) {
    const end = Math.min(offset + chunkSize, data.length);
    chunks.push(data.subarray(offset, end));
    offset = end;
  }

  let cell: Cell | null = null;
  for (let i = chunks.length - 1; i >= 0; i--) {
    const builder = beginCell();
    if (i === 0) {
      builder.storeUint(SNAKE_DATA_PREFIX, 8);
    }
    builder.storeBuffer(chunks[i]!);
    if (cell) {
      builder.storeRef(cell);
    }
    cell = builder.endCell();
  }
  return cell!;
}

export interface JettonMetadata {
  name: string;
  symbol: string;
  decimals: string;
  description?: string;
  image?: string;
  imageData?: string;
}

export async function buildOnchainMetadata(metadata: JettonMetadata): Promise<Cell> {
  const dict = Dictionary.empty(Dictionary.Keys.Buffer(32), Dictionary.Values.Cell());

  const entries: [string, string][] = [
    ['name', metadata.name],
    ['symbol', metadata.symbol],
    ['decimals', metadata.decimals],
  ];
  if (metadata.description) entries.push(['description', metadata.description]);
  if (metadata.image) entries.push(['image', metadata.image]);
  if (metadata.imageData) entries.push(['image_data', metadata.imageData]);

  for (const [key, value] of entries) {
    const keyHash = await sha256(key);
    const valueCell = makeSnakeCell(Buffer.from(value, 'utf-8'));
    dict.set(keyHash, valueCell);
  }

  return beginCell().storeUint(ONCHAIN_CONTENT_PREFIX, 8).storeDict(dict).endCell();
}
