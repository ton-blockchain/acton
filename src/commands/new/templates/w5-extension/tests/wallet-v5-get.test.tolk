import "@acton/emulation/network"
import "@acton/testing/expect"
import "@wrappers/WalletV5.gen"
import "@wrappers/utils"
import "@contracts/walletv5/storage"

get fun `test w5: get seqno`() {
    val expectedSeqno = 12345;
    var storage = defaultStorage();
    storage.seqno = expectedSeqno;

    val (walletV5, _) = deployWallet(storage);

    val actualSeqno = walletV5.getSeqno();
    expect(actualSeqno).toEqual(expectedSeqno);
}

get fun `test w5: get public key`() {
    val expectedPublicKey = 0x1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF;
    var storage = defaultStorage();
    storage.publicKey = expectedPublicKey;

    val (walletV5, _) = deployWallet(storage);

    val actualPublicKey = walletV5.getPublicKey();
    expect(actualPublicKey).toEqual(expectedPublicKey);
}

get fun `test w5: get subwallet id`() {
    val expectedSubwalletId = 12345;
    var storage = defaultStorage();
    storage.subwalletId = expectedSubwalletId;

    val (walletV5, _) = deployWallet(storage);

    val actualSubwalletId = walletV5.getSubwalletId();
    expect(actualSubwalletId).toEqual(expectedSubwalletId);
}

get fun `test w5: default subwallet id`() {
    val (walletV5, _) = deployWallet(defaultStorage());

    val actualSubwalletId = walletV5.getSubwalletId();
    expect(actualSubwalletId).toEqual(DEFAULT_SUBWALLET_ID);
}

get fun `test w5: get extensions dict`() {
    val plugin1 = randomAddress("plugin1");
    val plugin2 = randomAddress("plugin2");

    var extensions: ExtensionsDict = [];
    val (_, plugin1Hash) = plugin1.getWorkchainAndHash();
    val (_, plugin2Hash) = plugin2.getWorkchainAndHash();
    extensions.set(plugin1Hash, true);
    extensions.set(plugin2Hash, true);

    var storage = defaultStorage();
    storage.extensions = extensions;

    val (walletV5, _) = deployWallet(storage);

    val actual = walletV5.getExtensions();
    expect(actual).toEqual(extensions);
}

get fun `test w5: get extensions array`() {
    val plugin1 = randomAddress("plugin1");
    val plugin2 = randomAddress("plugin2");
    val plugin3 = randomAddress("plugin3");

    var extensions: ExtensionsDict = [];
    val (_, plugin1Hash) = plugin1.getWorkchainAndHash();
    val (_, plugin2Hash) = plugin2.getWorkchainAndHash();
    val (_, plugin3Hash) = plugin3.getWorkchainAndHash();
    extensions.set(plugin1Hash, true);
    extensions.set(plugin2Hash, true);
    extensions.set(plugin3Hash, true);

    var storage = defaultStorage();
    storage.extensions = extensions;

    val (walletV5, _) = deployWallet(storage);

    val actual = walletV5.getExtensionsArray();
    expect(actual.size()).toEqual(3);
    // Dictionary sorts by uint256 key (address hash), so order depends on hash values.
    // Check that all 3 plugins are present regardless of order.
    var foundPlugin1 = false;
    var foundPlugin2 = false;
    var foundPlugin3 = false;
    var i = 0;
    while (i < actual.size()) {
        if (actual.get(i) == plugin1) {
            foundPlugin1 = true;
        }
        if (actual.get(i) == plugin2) {
            foundPlugin2 = true;
        }
        if (actual.get(i) == plugin3) {
            foundPlugin3 = true;
        }
        i += 1;
    }
    expect(foundPlugin1).toEqual(true);
    expect(foundPlugin2).toEqual(true);
    expect(foundPlugin3).toEqual(true);
}

get fun `test w5: get empty extensions`() {
    val (walletV5, _) = deployWallet(defaultStorage());

    val actual = walletV5.getExtensions();
    expect(actual).toHaveLength(0);
}

get fun `test w5: get empty extensions array`() {
    val (walletV5, _) = deployWallet(defaultStorage());

    val actual = walletV5.getExtensionsArray();
    expect(actual.size()).toEqual(0);
}
