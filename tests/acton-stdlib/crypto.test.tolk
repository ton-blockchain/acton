import "@local-acton/crypto"
import "@local-acton/emulation/testing"
import "@local-acton/testing/expect"

get fun `test mnemonic new returns 24 words`() {
    val words = crypto.createMnemonic();
    expect(words.size()).toEqual(24);
}

get fun `test mnemonic to key pair and sign`() {
    val words = crypto.createMnemonic();
    val kp = words.toKeyPair();

    // privateKey and publicKey must be non-zero
    expect(kp.privateKey).toNotEqual(0);
    expect(kp.publicKey).toNotEqual(0);

    // Sign a cell and verify signature with isSignatureValid
    val data = beginCell().storeUint(12345, 64).endCell();
    val signature = crypto.sign(kp.privateKey, data);
    expect(signature).toBeNonEmpty();

    val hash = data.hash();
    val valid = isSignatureValid(hash, signature, kp.publicKey);
    expect(valid).toBeTrue();
}

get fun `test fast random bytes`() {
    // Same seed produce same output
    testing.setRandomSeed(42);
    val bytes1 = crypto.getFastRandomBytes(32);
    testing.setRandomSeed(42);
    val bytes2 = crypto.getFastRandomBytes(32);
    expect(bytes1).toEqual(bytes2);
}

get fun `test fast random bytes 2`() {
    // One seed two calls = different output
    testing.setRandomSeed(42);
    val bytes1 = crypto.getFastRandomBytes(32);
    val bytes2 = crypto.getFastRandomBytes(32);
    expect(bytes1).toNotEqual(bytes2);
}

get fun `test secure random bytes`() {
    val bytes1 = crypto.getSecureRandomBytes(32);
    val bytes2 = crypto.getSecureRandomBytes(32);

    // Two calls should produce different results (with overwhelming probability)
    expect(bytes1).toNotEqual(bytes2);
}

get fun `test generate key pair`() {
    val kp = crypto.generateKeyPair();

    expect(kp.privateKey).toNotEqual(0);
    expect(kp.publicKey).toNotEqual(0);

    // Generated key pair must produce valid signatures
    val data = beginCell().storeUint(0xDEAD, 16).endCell();
    val signature = crypto.sign(kp.privateKey, data);
    val valid = isSignatureValid(data.hash(), signature, kp.publicKey);
    expect(valid).toBeTrue();
}

get fun `test seed to key pair deterministic`() {
    // Same seed must produce the same key pair
    val seed = 12345;
    val kp1 = crypto.seedToKeyPair(seed);
    val kp2 = crypto.seedToKeyPair(seed);

    expect(kp1.privateKey).toEqual(kp2.privateKey);
    expect(kp1.publicKey).toEqual(kp2.publicKey);
}

get fun `test seed to key pair signs correctly`() {
    val seed = 99999;
    val kp = crypto.seedToKeyPair(seed);

    val data = beginCell().storeUint(0xCAFE, 16).endCell();
    val signature = crypto.sign(kp.privateKey, data);
    val valid = isSignatureValid(data.hash(), signature, kp.publicKey);
    expect(valid).toBeTrue();
}

get fun `test raw sign matches sign`() {
    val kp = crypto.generateKeyPair();
    val data = beginCell().storeUint(0xBEEF, 16).endCell();

    val rawSignature = crypto.rawSign(kp.privateKey, data.hash());
    val signature = crypto.sign(kp.privateKey, data);

    expect(rawSignature).toEqual(signature);
    expect(isSignatureValid(data.hash(), rawSignature, kp.publicKey)).toBeTrue();
}

get fun `test raw sign slice verifies under isSliceSignatureValid`() {
    val kp = crypto.seedToKeyPair(0xC0FFEE);

    // an arbitrary-length message (84 bytes — e.g. a validator block-signature
    // preimage), signed as raw bytes: the CHKSIGNS counterpart
    val message = beginCell()
        .storeUint(0xf83de3a8, 32)
        .storeUint(0xAAAA, 256)
        .storeUint(0x2c, 8)
        .storeUint(0x05e1a740, 32)
        .storeUint(0x3fcd91b6, 32)
        .storeUint(7, 32)
        .storeUint(0xBBBB, 256)
        .storeUint(0, 24)
        .toSlice();

    val signature = crypto.rawSignSlice(kp.privateKey, message);
    expect(isSliceSignatureValid(message, signature, kp.publicKey)).toBeTrue();

    // a different key must not verify
    val other = crypto.seedToKeyPair(0xDEAD);
    expect(isSliceSignatureValid(message, signature, other.publicKey)).toBeFalse();

    // a different message must not verify either
    val tampered = beginCell().storeUint(0xF00D, 32).toSlice();
    expect(isSliceSignatureValid(tampered, signature, kp.publicKey)).toBeFalse();
}

get fun `test raw sign slice of a 32-byte slice matches raw sign`() {
    val kp = crypto.generateKeyPair();
    val hash = beginCell().storeUint(0x1234, 16).endCell().hash();

    // Ed25519 is deterministic, and rawSign's message IS the 32 bytes of the hash —
    // so signing the same 32 bytes as a slice must produce the identical signature,
    // valid under both the CHKSIGNU and CHKSIGNS checks
    val sliceSignature = crypto.rawSignSlice(
        kp.privateKey,
        beginCell().storeUint(hash, 256).toSlice(),
    );
    expect(sliceSignature).toEqual(crypto.rawSign(kp.privateKey, hash));
    expect(isSignatureValid(hash, sliceSignature, kp.publicKey)).toBeTrue();
    expect(
        isSliceSignatureValid(
            beginCell().storeUint(hash, 256).toSlice(),
            sliceSignature,
            kp.publicKey,
        ),
    ).toBeTrue();
}
