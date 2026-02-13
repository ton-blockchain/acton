import "crypto"
import "../testing/expect"

get fun `test-mnemonic-new-returns-24-words`() {
    val words = crypto.createMnemonic();
    expect(words.size()).toEqual(24);
}

get fun `test-mnemonic-to-key-pair-and-sign`() {
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

get fun `test-fast-random-bytes`() {
    // Different seeds produce different output
    val bytes1 = crypto.getFastRandomBytes(32, 42);
    val bytes2 = crypto.getFastRandomBytes(32, 43);
    expect(bytes1).toNotEqual(bytes2);
}

get fun `test-secure-random-bytes`() {
    val bytes1 = crypto.getSecureRandomBytes(32);
    val bytes2 = crypto.getSecureRandomBytes(32);

    // Two calls should produce different results (with overwhelming probability)
    expect(bytes1).toNotEqual(bytes2);
}
