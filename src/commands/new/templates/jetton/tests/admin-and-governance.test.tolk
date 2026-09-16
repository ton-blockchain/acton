import "@acton/build"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/transaction"
import "@contracts/errors"
import "@contracts/messages"
import "@contracts/sharding"
import "@contracts/storage"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"
import "test-utils"

fun extractUriString(contentCell: cell): string? {
    val metadata = OnchainMetadataReply.fromCell(contentCell);
    val uriCell = metadata.contentDict.get("uri".sha256());
    if (!uriCell.isFound) {
        return null;
    }
    val uri = uriCell.loadValue().load();
    return uri.string;
}

fun assertSameShardPrefix(owner: address, wallet: address) {
    val (_, ownerHash) = owner.getWorkchainAndHash();
    val (_, walletHash) = wallet.getWorkchainAndHash();
    expect(ownerHash >> (256 - SHARD_DEPTH)).toEqual(walletHash >> (256 - SHARD_DEPTH));
}

get fun `test admin: deploy should create minter without bounce`() {
    val deployer = testing.treasury("deployer-manual-deploy");

    val minter = JettonMinter.fromStorage(MinterStorage {
        totalSupply: 0,
        adminAddress: deployer.address,
        metadata: buildTestMetadata("link"),
        nextAdminAddress: null,
    });

    val deployResult = minter.sendDeploy(deployer.address, { value: grams("10") });
    expect(deployResult).toHaveSuccessfulDeploy<TopUpGrams>({
        from: deployer.address,
        to: minter.address,
    });
    expect(deployResult).toNotHaveTx({
        from: minter.address,
        to: deployer.address,
        bounced: true,
    });
}

get fun `test admin: minter admin should be able to mint jettons`() {
    val (minter, deployer, notDeployer) = setupTest();
    val supplyBefore = minter.getTotalSupply();
    val deployerMint = grams("25");
    val otherMint = grams("7");

    val deployerWallet = mintTo(minter, deployer, deployer.address, deployerMint);
    val otherWallet = mintTo(minter, deployer, notDeployer.address, otherMint);

    expect(deployerWallet.getJettonBalance()).toEqual(deployerMint);
    expect(otherWallet.getJettonBalance()).toEqual(otherMint);
    expect(minter.getTotalSupply()).toEqual(supplyBefore + deployerMint + otherMint);
    assertSameShardPrefix(notDeployer.address, otherWallet.address);
}

get fun `test admin: non-admin should not be able to mint jettons`() {
    val (minter, deployer, notDeployer) = setupTest();
    val supplyBefore = minter.getTotalSupply();

    val mintResult = minter.sendMint(
        notDeployer.address,
        deployer.address,
        grams("5"),
        grams("0.05"),
        grams("0.3"),
    );
    expect(mintResult).toHaveFailedTx<MintNewJettons>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: ERROR_NOT_OWNER,
    });

    val deployerWallet = userWallet(minter, deployer.address);
    expect(deployerWallet.getJettonBalance()).toEqual(0);
    expect(minter.getTotalSupply()).toEqual(supplyBefore);
}

get fun `test admin: change-admin flow should work and only next admin can claim`() {
    val (minter, deployer, notDeployer) = setupTest();
    expect(minter.getAdminAddress()).toEqual(deployer.address);

    val changeResult = minter.sendChangeAdmin(deployer.address, notDeployer.address);
    expect(changeResult).toHaveSuccessfulTx<ChangeMinterAdmin>({
        from: deployer.address,
        to: minter.address,
    });
    expect(minter.getAdminAddress()).toEqual(deployer.address);

    val badClaim = minter.sendClaimAdmin(deployer.address);
    expect(badClaim).toHaveFailedTx<ClaimMinterAdmin>({
        from: deployer.address,
        to: minter.address,
        exitCode: ERROR_NOT_OWNER,
    });

    val claimResult = minter.sendClaimAdmin(notDeployer.address);
    expect(claimResult).toHaveSuccessfulTx<ClaimMinterAdmin>({
        from: notDeployer.address,
        to: minter.address,
    });
    expect(minter.getAdminAddress()).toEqual(notDeployer.address);

    minter.sendChangeAdmin(notDeployer.address, deployer.address);
    minter.sendClaimAdmin(deployer.address);
    expect(minter.getAdminAddress()).toEqual(deployer.address);
}

get fun `test admin: non-admin cannot change admin`() {
    val (minter, deployer, notDeployer) = setupTest();

    val changeResult = minter.sendChangeAdmin(notDeployer.address, deployer.address);
    expect(changeResult).toHaveFailedTx<ChangeMinterAdmin>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: ERROR_NOT_OWNER,
    });
    expect(minter.getAdminAddress()).toEqual(deployer.address);
}

get fun `test admin: non-admin cannot drop admin access`() {
    val (minter, _, notDeployer) = setupTest();

    val dropResult = minter.sendDropAdmin(notDeployer.address);
    expect(dropResult).toHaveFailedTx<DropMinterAdmin>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: ERROR_NOT_OWNER,
    });
}

get fun `test admin: admin can drop admin access`() {
    val (minter, deployer, _) = setupTest();
    expect(minter.getAdminAddress()).toEqual(deployer.address);

    val dropResult = minter.sendDropAdmin(deployer.address);
    expect(dropResult).toHaveSuccessfulTx<DropMinterAdmin>({
        from: deployer.address,
        to: minter.address,
    });
    expect(minter.getAdminAddress()).toEqual(null);
}

get fun `test admin: admin can change metadata uri`() {
    val (minter, deployer, _) = setupTest();
    val newUri = "https://new.meta.example/jetton.json";

    val oldUri = extractUriString(minter.getContent());
    expect(oldUri).toBeNotNull();

    val changeResult = minter.sendChangeMetadata(deployer.address, buildTestMetadata(newUri));
    expect(changeResult).toHaveSuccessfulTx<ChangeMinterMetadata>({
        from: deployer.address,
        to: minter.address,
    });

    val updatedUri = extractUriString(minter.getContent());
    expect(updatedUri).toBeNotNull();
    if (updatedUri == null) {
        return;
    }
    expect(updatedUri).toEqual(newUri);
}

get fun `test admin: non-admin cannot change metadata uri`() {
    val (minter, _, notDeployer) = setupTest();
    val oldContentHash = minter.getContent().hash();

    val changeResult = minter.sendChangeMetadata(
        notDeployer.address,
        buildTestMetadata("https://evil.meta.example/jetton.json"),
    );
    expect(changeResult).toHaveFailedTx<ChangeMinterMetadata>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: ERROR_NOT_OWNER,
    });
    expect(minter.getContent().hash()).toEqual(oldContentHash);
}

get fun `test admin: discovery should return expected wallet and optional owner`() {
    val (minter, deployer, notDeployer) = setupTest();
    mintTo(minter, deployer, deployer.address, grams("1"));

    val expectedDeployerWallet = userWallet(minter, deployer.address).address;
    val withOwnerRes = minter.sendDiscovery(deployer.address, deployer.address, true);
    val withOwnerTx = withOwnerRes.findTransaction<ResponseWalletAddress>({
        from: minter.address,
        to: deployer.address,
        success: true,
        exitCode: 0,
    });
    expect(withOwnerTx).toBeNotNull();
    val withOwnerBody = withOwnerTx!.loadBody<ResponseWalletAddress>();
    expect(withOwnerBody.jettonWalletAddress).toEqual(expectedDeployerWallet);
    expect(withOwnerBody.ownerAddress).toBeNotNull();
    if (withOwnerBody.ownerAddress == null) {
        return;
    }
    expect(withOwnerBody.ownerAddress.load()).toEqual(deployer.address);

    val expectedOtherWallet = userWallet(minter, notDeployer.address).address;
    val withoutOwnerRes = minter.sendDiscovery(deployer.address, notDeployer.address, false);
    val withoutOwnerTx = withoutOwnerRes.findTransaction<ResponseWalletAddress>({
        from: minter.address,
        to: deployer.address,
        success: true,
        exitCode: 0,
    });
    expect(withoutOwnerTx).toBeNotNull();
    val withoutOwnerBody = withoutOwnerTx!.loadBody<ResponseWalletAddress>();
    expect(withoutOwnerBody.jettonWalletAddress).toEqual(expectedOtherWallet);
    expect(withoutOwnerBody.ownerAddress).toEqual(null);
}

get fun `test admin: discovery should return none for non-basechain owner`() {
    val (minter, deployer, _) = setupTest();
    val invalidOwner = address.fromWorkchainAndHash(-1, 0);

    val noOwnerRes = minter.sendDiscovery(deployer.address, invalidOwner, false);
    val noOwnerTx = noOwnerRes.findTransaction<ResponseWalletAddress>({
        from: minter.address,
        to: deployer.address,
        success: true,
        exitCode: 0,
    });
    expect(noOwnerTx).toBeNotNull();
    val noOwnerBody = noOwnerTx!.loadBody<ResponseWalletAddress>();
    expect(noOwnerBody.jettonWalletAddress).toEqual(null);
    expect(noOwnerBody.ownerAddress).toEqual(null);

    val withOwnerRes = minter.sendDiscovery(deployer.address, invalidOwner, true);
    val withOwnerTx = withOwnerRes.findTransaction<ResponseWalletAddress>({
        from: minter.address,
        to: deployer.address,
        success: true,
        exitCode: 0,
    });
    expect(withOwnerTx).toBeNotNull();
    val withOwnerBody = withOwnerTx!.loadBody<ResponseWalletAddress>();
    expect(withOwnerBody.jettonWalletAddress).toEqual(null);
    expect(withOwnerBody.ownerAddress).toBeNotNull();
    if (withOwnerBody.ownerAddress == null) {
        return;
    }
    expect(withOwnerBody.ownerAddress.load()).toEqual(invalidOwner);
}

get fun `test admin: non-admin should not be able to upgrade minter`() {
    val (minter, deployer, notDeployer) = setupTest();
    val newData = MinterStorage {
        totalSupply: grams("99"),
        adminAddress: deployer.address,
        nextAdminAddress: null,
        metadata: buildTestMetadata("meta-upgrade"),
    }.toCell();
    val newCode = build("JettonMinter");
    val oldContentHash = minter.getContent().hash();

    val res = minter.sendUpgrade(notDeployer.address, newCode, newData);
    expect(res).toHaveFailedTx<UpgradeMinterCode>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: ERROR_NOT_OWNER,
    });
    expect(minter.getContent().hash()).toEqual(oldContentHash);
}

get fun `test admin: admin should be able to upgrade minter code and data`() {
    val (minter, deployer, _) = setupTest();
    val upgradedSupply = grams("42");
    val upgradedUri = "https://upgraded.meta.example/jetton.json";
    val newData = MinterStorage {
        totalSupply: upgradedSupply,
        adminAddress: deployer.address,
        nextAdminAddress: null,
        metadata: buildTestMetadata(upgradedUri),
    }.toCell();
    val newCode = build("JettonMinter");

    val res = minter.sendUpgrade(deployer.address, newCode, newData);
    expect(res).toHaveSuccessfulTx<UpgradeMinterCode>({
        from: deployer.address,
        to: minter.address,
    });

    expect(minter.getTotalSupply()).toEqual(upgradedSupply);
    expect(minter.getAdminAddress()).toEqual(deployer.address);

    val uri = extractUriString(minter.getContent());
    expect(uri).toBeNotNull();
    if (uri == null) {
        return;
    }
    expect(uri).toEqual(upgradedUri);
}

get fun `test shard: wallet address should keep owner shard prefix in get/discovery/transfer`() {
    val (minter, deployer, _) = setupTest();
    val senderWallet = mintTo(minter, deployer, deployer.address, grams("100"));

    var prefix = 0;
    while (prefix < 32) {
        val owner = address.fromWorkchainAndHash(0, prefix << 248);
        val expectedWalletAddress = minter.getWalletAddress(owner);
        assertSameShardPrefix(owner, expectedWalletAddress);

        val discovery = minter.sendDiscovery(deployer.address, owner, true);
        val discoveryTx = discovery.findTransaction<ResponseWalletAddress>({
            from: minter.address,
            to: deployer.address,
            success: true,
            exitCode: 0,
        });
        expect(discoveryTx).toBeNotNull();
        val discoveryBody = discoveryTx!.loadBody<ResponseWalletAddress>();
        expect(discoveryBody.jettonWalletAddress).toEqual(expectedWalletAddress);

        val transfer = senderWallet.sendTransferPayloadInline(
            deployer.address,
            DEFAULT_TRANSFER_MSG_VALUE,
            TEST_JETTON_UNIT,
            owner,
            deployer.address,
            null,
            0,
            createEmptySlice(),
        );
        expect(transfer).toHaveSuccessfulTx<AskToTransfer>({
            from: deployer.address,
            to: senderWallet.address,
        });
        expect(transfer).toHaveSuccessfulTx<InternalTransferStep>({
            from: senderWallet.address,
            to: expectedWalletAddress,
        });

        prefix += 1;
    }
}
