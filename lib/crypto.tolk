/// Module for cryptographic operations.
///
/// This module provides functions for generating mnemonics, deriving key pairs,
/// signing data, and generating random bytes.
///
/// Examples:
/// ```tolk
/// // Generate a new mnemonic and derive a key pair
/// val words = crypto.createMnemonic();
/// val kp = words.toKeyPair();
///
/// // Sign a cell
/// val signature = crypto.sign(kp.privateKey, myCell);
///
/// // Generate random bytes
/// val randomBytes = crypto.getSecureRandomBytes(32);
/// ```

import "ffi"
import "testing/assert"

struct crypto

/// Represents an Ed25519 key pair.
///
/// The `privateKey` is a 256-bit seed used to derive the full signing key.
/// The `publicKey` is the corresponding 256-bit Ed25519 public key.
///
/// Both fields are plain `int` for compatibility with TVM builtins like [isSignatureValid].
struct KeyPair {
    privateKey: int
    publicKey: int
}

// Mnemonic phrase represented as `array<string>` (24 words for `crypto.createMnemonic()`).
type Mnemonic = array<string>

/// Generates a new random BIP39-compatible mnemonic phrase.
///
/// Returns an array of 24 English words that can be used to derive a key pair
/// via [Mnemonic.toKeyPair].
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
/// `self` must be a [Mnemonic] (`array<string>`) of 24 valid BIP39 words,
/// for example as returned by [crypto.createMnemonic].
///
/// Example:
/// ```tolk
/// val words = crypto.createMnemonic();
/// val kp = words.toKeyPair();
/// // kp.privateKey — 32-byte seed
/// // kp.publicKey  — 32-byte public key
/// ```
fun Mnemonic.toKeyPair(self): KeyPair {
    val [privateKey, publicKey] = ffi.mnemonicToKeyPair(self);
    return KeyPair { privateKey, publicKey };
}

/// Signs a cell by hashing it and then signing the hash.
///
/// This is a convenience wrapper around [crypto.rawSign] that first computes
/// the cell hash and then signs it with the given private key.
/// Returns a 512-bit (64-byte) Ed25519 signature as a slice.
///
/// Example:
/// ```tolk
/// val kp = words.toKeyPair();
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
/// val kp = words.toKeyPair();
/// val hash = myCell.hash();
/// val signature = crypto.rawSign(kp.privateKey, hash);
/// ```
fun crypto.rawSign(privateKey: int, data: int): slice {
    return ffi.rawSign(privateKey, data);
}

/// Generates cryptographically secure random bytes.
///
/// Returns a slice containing `bytesNum` random bytes.
///
/// Current accepted range is `1..127`.
/// `0` and values `>= 128` fail.
///
/// Example:
/// ```tolk
/// val randomBytes = crypto.getSecureRandomBytes(32);
/// ```
fun crypto.getSecureRandomBytes(bytesNum: uint16): slice {
    if (bytesNum >= 128) {
        Assert.fail("getSecureRandomBytes expects bytesNum less than 128");
    }
    return ffi.getSecureRandomBytes(bytesNum);
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
    if (seed == null) {
        seed = blockchain.now();
    }
    random.setSeed(seed as uint256);
    // In this case bytes won't fit in one slice
    if (bytesNum >= 128) {
        Assert.fail("getFastRandomBytes expects bytesNum less than 128");
    }

    var res = beginCell();
    while (bytesNum > 0) {
        var rnd = beginCell().storeUint(random.uint256(), 256).toSlice();
        val neededBytesNum = min(32, bytesNum);
        res.storeUint(rnd.loadUint(8 * neededBytesNum), 8 * neededBytesNum);
        bytesNum -= neededBytesNum;
    }
    return res.toSlice();
}

/// Generates a random Ed25519 [KeyPair] without creating a mnemonic.
///
/// Much faster than [crypto.createMnemonic] + [Mnemonic.toKeyPair] because it
/// skips BIP39 word generation.  Use this when you don't need the mnemonic itself.
///
/// Example:
/// ```tolk
/// val kp = crypto.generateKeyPair();
/// ```
fun crypto.generateKeyPair(): KeyPair {
    val seed = crypto.getSecureRandomBytes(32).loadUint(256);
    return crypto.seedToKeyPair(seed);
}

/// Derives an Ed25519 [KeyPair] from a 256-bit seed.
/// The seed is used as the Ed25519 private key seed; the corresponding public
/// key is computed deterministically.  The same seed always yields the same key pair.
///
/// Example:
/// ```tolk
/// val kp = crypto.seedToKeyPair(mySeed);
/// ```
fun crypto.seedToKeyPair(seed: uint256): KeyPair {
    val [privateKey, publicKey] = ffi.seedToKeyPair(seed);
    return KeyPair { privateKey, publicKey };
}

fun ffi.seedToKeyPair(seed: uint256): [int, int] asm "404 EXTCALL";
fun ffi.createMnemonic(): Mnemonic asm "401 EXTCALL"
fun ffi.mnemonicToKeyPair(words: Mnemonic): [int, int] asm "402 EXTCALL";
fun ffi.getSecureRandomBytes(bytesNum: uint16): slice asm "400 EXTCALL"
fun ffi.rawSign(privateKey: int, data: int): slice asm "403 EXTCALL";
