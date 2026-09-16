import { beginCell, Cell, Dictionary } from '@ton/core';
import { SnakeDataReply } from '@wrappers/JettonMinter.gen';

const ONCHAIN_CONTENT_PREFIX = 0x00;

const sha256Keys: Record<string, Buffer> = {};

async function sha256(key: string): Promise<Buffer> {
  if (!sha256Keys[key]) {
    const data = new TextEncoder().encode(key);
    const hash = await crypto.subtle.digest('SHA-256', data);
    sha256Keys[key] = Buffer.from(hash);
  }
  return sha256Keys[key]!;
}

export interface JettonMetadata {
  name: string;
  symbol: string;
  decimals: string;
  description?: string;
  image?: string;
  imageData?: string;
}

export async function buildOnchainMetadata(
  metadata: JettonMetadata,
): Promise<Cell> {
  const dict = Dictionary.empty(
    Dictionary.Keys.Buffer(32),
    Dictionary.Values.Cell(),
  );

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
    const valueCell = SnakeDataReply.toCell(
      SnakeDataReply.create({ string: value }),
    );
    dict.set(keyHash, valueCell);
  }

  return beginCell()
    .storeUint(ONCHAIN_CONTENT_PREFIX, 8)
    .storeDict(dict)
    .endCell();
}
