import type {SourceBundle, SourceFile} from "./api"

const encoder = new TextEncoder()
const ZIP_UTF8_FLAG = 0x08_00
const ZIP_STORE_METHOD = 0
const ZIP_VERSION = 20
const ZIP_DOS_TIME = 0
const ZIP_DOS_DATE = 33

interface ZipEntry {
  readonly path: string
  readonly pathBytes: Uint8Array
  readonly data: Uint8Array
  readonly crc32: number
  readonly localHeaderOffset: number
}

const crcTable = new Uint32Array(256)

for (let index = 0; index < crcTable.length; index += 1) {
  let value = index
  for (let bit = 0; bit < 8; bit += 1) {
    value = value & 1 ? 0xed_b8_83_20 ^ (value >>> 1) : value >>> 1
  }
  crcTable[index] = value >>> 0
}

function crc32(data: Uint8Array): number {
  let value = 0xff_ff_ff_ff
  for (const byte of data) {
    value = crcTable[(value ^ byte) & 0xff] ^ (value >>> 8)
  }
  return (value ^ 0xff_ff_ff_ff) >>> 0
}

function writeUint16(buffer: Uint8Array, offset: number, value: number): void {
  buffer[offset] = value & 0xff
  buffer[offset + 1] = (value >>> 8) & 0xff
}

function writeUint32(buffer: Uint8Array, offset: number, value: number): void {
  buffer[offset] = value & 0xff
  buffer[offset + 1] = (value >>> 8) & 0xff
  buffer[offset + 2] = (value >>> 16) & 0xff
  buffer[offset + 3] = (value >>> 24) & 0xff
}

function sourceFileBytes(file: SourceFile): Uint8Array {
  return encoder.encode(file.content)
}

function archivePath(path: string, fallbackIndex: number): string {
  const parts = path.replace(/\\/g, "/").split("/")
  const safeParts = parts.filter(part => part.length > 0 && part !== "." && part !== "..")
  return safeParts.join("/") || `source-${fallbackIndex + 1}`
}

function writeLocalHeader(output: Uint8Array, offset: number, entry: ZipEntry): number {
  writeUint32(output, offset, 0x04_03_4b_50)
  writeUint16(output, offset + 4, ZIP_VERSION)
  writeUint16(output, offset + 6, ZIP_UTF8_FLAG)
  writeUint16(output, offset + 8, ZIP_STORE_METHOD)
  writeUint16(output, offset + 10, ZIP_DOS_TIME)
  writeUint16(output, offset + 12, ZIP_DOS_DATE)
  writeUint32(output, offset + 14, entry.crc32)
  writeUint32(output, offset + 18, entry.data.length)
  writeUint32(output, offset + 22, entry.data.length)
  writeUint16(output, offset + 26, entry.pathBytes.length)
  writeUint16(output, offset + 28, 0)
  output.set(entry.pathBytes, offset + 30)
  output.set(entry.data, offset + 30 + entry.pathBytes.length)
  return offset + 30 + entry.pathBytes.length + entry.data.length
}

function writeCentralDirectoryHeader(output: Uint8Array, offset: number, entry: ZipEntry): number {
  writeUint32(output, offset, 0x02_01_4b_50)
  writeUint16(output, offset + 4, ZIP_VERSION)
  writeUint16(output, offset + 6, ZIP_VERSION)
  writeUint16(output, offset + 8, ZIP_UTF8_FLAG)
  writeUint16(output, offset + 10, ZIP_STORE_METHOD)
  writeUint16(output, offset + 12, ZIP_DOS_TIME)
  writeUint16(output, offset + 14, ZIP_DOS_DATE)
  writeUint32(output, offset + 16, entry.crc32)
  writeUint32(output, offset + 20, entry.data.length)
  writeUint32(output, offset + 24, entry.data.length)
  writeUint16(output, offset + 28, entry.pathBytes.length)
  writeUint16(output, offset + 30, 0)
  writeUint16(output, offset + 32, 0)
  writeUint16(output, offset + 34, 0)
  writeUint16(output, offset + 36, 0)
  writeUint32(output, offset + 38, 0)
  writeUint32(output, offset + 42, entry.localHeaderOffset)
  output.set(entry.pathBytes, offset + 46)
  return offset + 46 + entry.pathBytes.length
}

export function sourceArchiveName(bundle: SourceBundle): string {
  return `sources-${bundle.source_bundle_hash.slice(0, 12)}.zip`
}

export function buildSourceArchive(bundle: SourceBundle): Blob {
  let localOffset = 0
  const entries = bundle.files.map((file, index) => {
    const path = archivePath(file.path, index)
    const pathBytes = encoder.encode(path)
    const data = sourceFileBytes(file)
    const entry: ZipEntry = {
      path,
      pathBytes,
      data,
      crc32: crc32(data),
      localHeaderOffset: localOffset,
    }
    localOffset += 30 + pathBytes.length + data.length
    return entry
  })

  const centralDirectoryOffset = localOffset
  const centralDirectorySize = entries.reduce(
    (size, entry) => size + 46 + entry.pathBytes.length,
    0,
  )
  const output = new Uint8Array(centralDirectoryOffset + centralDirectorySize + 22)
  let offset = 0

  for (const entry of entries) {
    offset = writeLocalHeader(output, offset, entry)
  }

  for (const entry of entries) {
    offset = writeCentralDirectoryHeader(output, offset, entry)
  }

  writeUint32(output, offset, 0x06_05_4b_50)
  writeUint16(output, offset + 4, 0)
  writeUint16(output, offset + 6, 0)
  writeUint16(output, offset + 8, entries.length)
  writeUint16(output, offset + 10, entries.length)
  writeUint32(output, offset + 12, centralDirectorySize)
  writeUint32(output, offset + 16, centralDirectoryOffset)
  writeUint16(output, offset + 20, 0)

  return new Blob([output], {type: "application/zip"})
}

export function downloadSourceArchive(bundle: SourceBundle): void {
  const url = URL.createObjectURL(buildSourceArchive(bundle))
  const link = document.createElement("a")
  link.href = url
  link.download = sourceArchiveName(bundle)
  document.body.append(link)
  link.click()
  link.remove()
  globalThis.setTimeout(() => URL.revokeObjectURL(url), 0)
}
