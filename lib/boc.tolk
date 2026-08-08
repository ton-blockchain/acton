/// Module for encoding cells as raw Bag of Cells (BoC) bytes and decoding them back.
///
/// The encoded bytes can be written directly with `fs.writeBytes` or passed to APIs that
/// expect a binary BoC. Decoding accepts standard single-root BoCs with or without CRC32C
/// and with or without an index.
///
/// Example:
/// ```tolk
/// val data = beginCell()
///     .storeUint(0xDEADBEEF, 32)
///     .endCell();
///
/// val encoded = boc.encode(data, { crc32: true });
/// fs.writeBytes("data.boc", encoded);
///
/// val decoded = boc.decode(fs.readBytes("data.boc")!);
/// if (decoded != null) {
///     // decoded! contains the original cell tree
/// }
/// ```

import "ffi"

/// Namespace for raw Bag of Cells encoding and decoding.
struct boc

/// Options controlling raw BoC encoding.
struct BocEncodeOptions {
    /// Append a four-byte CRC32C checksum to the encoded BoC.
    crc32: bool = false
}

/// Encodes a cell or the unread remainder of a slice as raw BoC bytes.
///
/// Returns a snake `slice` containing the exact binary representation. By default, the BoC
/// has one root, no index, and no checksum. Set `options.crc32` to append CRC32C.
///
/// When `value` is a `slice`, only its remaining bits and references are encoded. Bits or
/// references already loaded from the slice are not included.
fun boc.encode<T>(value: T, options: BocEncodeOptions = {}): slice {
    return value.__bocEncode(options.crc32);
}

/// Decodes raw BoC bytes into a single root cell.
///
/// Returns `null` when `data` is not a valid snake byte slice, is not a valid BoC, has an
/// invalid CRC32C checksum, or contains a number of roots other than one.
fun boc.decode(data: slice): cell? {
    return ffi.bocDecode(data);
}

fun cell.__bocEncode(self, crc32: bool): slice {
    return ffi.bocEncode(self, crc32);
}

fun slice.__bocEncode(self, crc32: bool): slice {
    return ffi.bocEncode(self, crc32);
}

fun Cell<T>.__bocEncode(self, crc32: bool): slice {
    return ffi.bocEncode(self, crc32);
}

fun ffi.bocEncode<T>(value: T, crc32: bool): slice
    asm "62 EXTCALL"

fun ffi.bocDecode(data: slice): cell?
    asm "63 EXTCALL"
