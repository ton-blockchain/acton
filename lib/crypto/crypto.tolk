/// Module for cryptographic operations.
///
/// This module provides functions for generating mnemonics, deriving key pairs,
/// signing data, and generating random bytes.
///
/// Examples:
/// ```tolk
/// // Generate a new mnemonic and derive a key pair
/// val words = crypto.mnemonicNew();
/// val kp = crypto.mnemonicToKeyPair(words);
///
/// // Sign a cell
/// val signature = crypto.sign(kp.privateKey, myCell);
///
/// // Generate random bytes
/// val randomBytes = crypto.getSecureRandomBytes(32);
/// ```

import "../ffi/ffi"
import "../testing/assert"

struct crypto

/// Represents an Ed25519 key pair.
///
/// The `privateKey` is a 256-bit seed used to derive the full signing key.
/// The `publicKey` is the corresponding 256-bit Ed25519 public key.
///
/// Both fields are plain `int` for compatibility with TVM builtins like [isSignatureValid].
struct KeyPair {
    privateKey: int;
    publicKey: int;
}

// With an assumption, that the length of an array is exactly 24
type Mnemonic = array<string>

/// Generates a new random BIP39-compatible mnemonic phrase.
///
/// Returns an array of 24 English words that can be used to derive a key pair
/// via [crypto.mnemonicToKeyPair].
///
/// Example:
/// ```tolk
/// val words = crypto.createMnemonic();
/// // words is an array of 24 strings
/// ```
fun crypto.createMnemonic(): Mnemonic {
    return ffi.createMnemonic();
}

/// Derives an Ed25519 [KeyPair] from a mnemonic phrase.
///
/// The `words` parameter must be a tuple of 24 valid BIP39 mnemonic words,
/// as returned by [crypto.mnemonicNew].
///
/// Example:
/// ```tolk
/// val words = crypto.mnemonicNew();
/// val kp = crypto.mnemonicToKeyPair(words);
/// // kp.privateKey — 32-byte seed
/// // kp.publicKey  — 32-byte public key
/// ```
fun Mnemonic.toKeyPair(self): KeyPair {
    val [privateKey, publicKey] = ffi.__mnemonicToKeyPair(self);
    return KeyPair {
        privateKey,
        publicKey,
    };
}

/// Signs a cell by hashing it and then signing the hash.
///
/// This is a convenience wrapper around [crypto.rawSign] that first computes
/// the cell hash and then signs it with the given private key.
/// Returns a 512-bit (64-byte) Ed25519 signature as a slice.
///
/// Example:
/// ```tolk
/// val kp = crypto.mnemonicToKeyPair(words);
/// val signature = crypto.sign(kp.privateKey, myCell);
/// ```
fun crypto.sign(privateKey: int, data: cell): slice {
    return crypto.rawSign(privateKey, data.hash());
}

/// Signs a raw 256-bit hash with the given private key using Ed25519.
///
/// Returns a 512-bit (64-byte) Ed25519 signature as a slice.
/// The `privateKey` must be a 32-byte seed as int
///
/// Example:
/// ```tolk
/// val kp = crypto.mnemonicToKeyPair(words);
/// val hash = myCell.hash();
/// val signature = crypto.rawSign(kp.privateKey, hash);
/// ```
fun crypto.rawSign(privateKey: int, data: int): slice {
    return ffi.rawSign(privateKey, data);
}

/// Generates cryptographically secure random bytes.
///
/// Returns a slice containing `bytesNum` random bytes (1 to 128).
///
/// Example:
/// ```tolk
/// val randomBytes = crypto.getSecureRandomBytes(32);
/// ```
fun crypto.getSecureRandomBytes(bytesNum: uint16): slice {
    if (bytesNum >= 128) {
        Assert.fail("getSecureRandomBytes expects bytesNum less than 128");
    }
    return ffi.__getSecureRandomBytes(bytesNum);
}

/// Generates fast (non-cryptographic) deterministic random bytes using a seed.
///
/// If `seed` is not provided, the current blockchain time is used.
/// The maximum `bytesNum` is 127; values >= 128 will throw an error
///
/// Same seed always produces the same output, which is useful for reproducible tests.
///
/// Example:
/// ```tolk
/// val bytes = crypto.getFastRandomBytes(16, 42);
/// ```
fun crypto.getFastRandomBytes(bytesNum: uint16, seed: int? = null): slice {
    if(seed == null) {
        seed = blockchain.now();
    }
    random.initializeBy(seed);
    // In this case bytes won't fit in one slice
    if (bytesNum >= 128) {
        Assert.fail("getFastRandomBytes expects bytesNum less than 128");
    }

    var res = beginCell();
    while(bytesNum > 0) {
        var rnd = beginCell().storeUint(random.uint256(), 256).toSlice();
        val neededBytesNum = min(32, bytesNum);
        res.storeUint(rnd.loadUint(8 * neededBytesNum), 8 * neededBytesNum);
        bytesNum -= neededBytesNum;
    }
    return res.toSlice();
}

fun ffi.createMnemonic(): Mnemonic asm "401 EXTCALL"
fun ffi.__mnemonicToKeyPair(words: Mnemonic): [int, int] asm "402 EXTCALL";
fun ffi.__getSecureRandomBytes(bytesNum: uint16): slice asm "400 EXTCALL"
fun ffi.rawSign(privateKey: int, data: int): slice asm "403 EXTCALL";
