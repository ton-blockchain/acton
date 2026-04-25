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

    val deployResult = minter.sendDeploy(deployer.address, { value: ton("10") });
    expect(deployResult).toHaveSuccessfulDeploy<TopUpTons>({
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
    val deployerMint = ton("25");
    val otherMint = ton("7");

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
        ton("5"),
        ton("0.05"),
        ton("0.3"),
    );
    expect(mintResult).toHaveFailedTx<MintNewJettons>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: Errors.NotOwner as int32,
    });

    val deployerWallet = userWallet(minter, deployer.address);
    expect(deployerWallet.getJettonBalance()).toEqual(0);
    expect(minter.getTotalSupply()).toEqual(supplyBefore);
}

get fun `test admin: change-admin flow should work and only next admin can claim`() {
    val (minter, deployer, notDeployer) = setupTest();
    expect(minter.getAdminAddress()).toEqual(deployer.address);

    val changeResult = minter.sendChangeMinterAdmin(
        deployer.address,
        0,
        notDeployer.address,
        { bounce: false, value: ton("0.05") },
    );
    expect(changeResult).toHaveSuccessfulTx<ChangeMinterAdmin>({
        from: deployer.address,
        to: minter.address,
    });
    expect(minter.getAdminAddress()).toEqual(deployer.address);

    val badClaim = minter.sendClaimMinterAdmin(
        deployer.address,
        0,
        { bounce: false, value: ton("0.05") },
    );
    expect(badClaim).toHaveFailedTx<ClaimMinterAdmin>({
        from: deployer.address,
        to: minter.address,
        exitCode: Errors.NotOwner as int32,
    });

    val claimResult = minter.sendClaimMinterAdmin(
        notDeployer.address,
        0,
        { bounce: false, value: ton("0.05") },
    );
    expect(claimResult).toHaveSuccessfulTx<ClaimMinterAdmin>({
        from: notDeployer.address,
        to: minter.address,
    });
    expect(minter.getAdminAddress()).toEqual(notDeployer.address);

    minter.sendChangeMinterAdmin(
        notDeployer.address,
        0,
        deployer.address,
        { bounce: false, value: ton("0.05") },
    );
    minter.sendClaimMinterAdmin(deployer.address, 0, { bounce: false, value: ton("0.05") });
    expect(minter.getAdminAddress()).toEqual(deployer.address);
}

get fun `test admin: non-admin cannot change admin`() {
    val (minter, deployer, notDeployer) = setupTest();

    val changeResult = minter.sendChangeMinterAdmin(
        notDeployer.address,
        0,
        deployer.address,
        { bounce: false, value: ton("0.05") },
    );
    expect(changeResult).toHaveFailedTx<ChangeMinterAdmin>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: Errors.NotOwner as int32,
    });
    expect(minter.getAdminAddress()).toEqual(deployer.address);
}

get fun `test admin: non-admin cannot drop admin access`() {
    val (minter, _, notDeployer) = setupTest();

    val dropResult = minter.sendDropMinterAdmin(
        notDeployer.address,
        0,
        { bounce: false, value: ton("0.05") },
    );
    expect(dropResult).toHaveFailedTx<DropMinterAdmin>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: Errors.NotOwner as int32,
    });
}

get fun `test admin: admin can drop admin access`() {
    val (minter, deployer, _) = setupTest();
    expect(minter.getAdminAddress()).toEqual(deployer.address);

    val dropResult = minter.sendDropMinterAdmin(
        deployer.address,
        0,
        { bounce: false, value: ton("0.05") },
    );
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

    val changeResult = minter.sendChangeMinterMetadata(
        deployer.address,
        0,
        buildTestMetadata(newUri),
        { bounce: false, value: ton("0.05") },
    );
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

    val changeResult = minter.sendChangeMinterMetadata(
        notDeployer.address,
        0,
        buildTestMetadata("https://evil.meta.example/jetton.json"),
        { bounce: false, value: ton("0.05") },
    );
    expect(changeResult).toHaveFailedTx<ChangeMinterMetadata>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: Errors.NotOwner as int32,
    });
    expect(minter.getContent().hash()).toEqual(oldContentHash);
}

get fun `test admin: discovery should return expected wallet and optional owner`() {
    val (minter, deployer, notDeployer) = setupTest();
    mintTo(minter, deployer, deployer.address, ton("1"));

    val expectedDeployerWallet = userWallet(minter, deployer.address).address;
    val withOwnerRes = minter.sendRequestWalletAddress(
        deployer.address,
        0,
        deployer.address,
        true,
        { bounce: false, value: ton("0.1") },
    );
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
    val withoutOwnerRes = minter.sendRequestWalletAddress(
        deployer.address,
        0,
        notDeployer.address,
        false,
        { bounce: false, value: ton("0.1") },
    );
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

    val noOwnerRes = minter.sendRequestWalletAddress(
        deployer.address,
        0,
        invalidOwner,
        false,
        { bounce: false, value: ton("0.1") },
    );
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

    val withOwnerRes = minter.sendRequestWalletAddress(
        deployer.address,
        0,
        invalidOwner,
        true,
        { bounce: false, value: ton("0.1") },
    );
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
        totalSupply: ton("99"),
        adminAddress: deployer.address,
        nextAdminAddress: null,
        metadata: buildTestMetadata("meta-upgrade"),
    }.toCell();
    val newCode = build("JettonMinter");
    val oldContentHash = minter.getContent().hash();

    val res = minter.sendUpgradeMinterCode(
        notDeployer.address,
        0,
        newData,
        newCode,
        { bounce: false, value: ton("0.2") },
    );
    expect(res).toHaveFailedTx<UpgradeMinterCode>({
        from: notDeployer.address,
        to: minter.address,
        exitCode: Errors.NotOwner as int32,
    });
    expect(minter.getContent().hash()).toEqual(oldContentHash);
}

get fun `test admin: admin should be able to upgrade minter code and data`() {
    val (minter, deployer, _) = setupTest();
    val upgradedSupply = ton("42");
    val upgradedUri = "https://upgraded.meta.example/jetton.json";
    val newData = MinterStorage {
        totalSupply: upgradedSupply,
        adminAddress: deployer.address,
        nextAdminAddress: null,
        metadata: buildTestMetadata(upgradedUri),
    }.toCell();
    val newCode = build("JettonMinter");

    val res = minter.sendUpgradeMinterCode(
        deployer.address,
        0,
        newData,
        newCode,
        { bounce: false, value: ton("0.2") },
    );
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
    val senderWallet = mintTo(minter, deployer, deployer.address, ton("100"));

    var prefix = 0;
    while (prefix < 32) {
        val owner = address.fromWorkchainAndHash(0, prefix << 248);
        val expectedWalletAddress = minter.getWalletAddress(owner);
        assertSameShardPrefix(owner, expectedWalletAddress);

        val discovery = minter.sendRequestWalletAddress(
            deployer.address,
            0,
            owner,
            true,
            { bounce: false, value: ton("0.1") },
        );
        val discoveryTx = discovery.findTransaction<ResponseWalletAddress>({
            from: minter.address,
            to: deployer.address,
            success: true,
            exitCode: 0,
        });
        expect(discoveryTx).toBeNotNull();
        val discoveryBody = discoveryTx!.loadBody<ResponseWalletAddress>();
        expect(discoveryBody.jettonWalletAddress).toEqual(expectedWalletAddress);

        val transfer = senderWallet.sendTransfer(
            deployer.address,
            DEFAULT_TRANSFER_MSG_VALUE,
            TEST_JETTON_UNIT,
            owner,
            deployer.address,
            null,
            0,
            null,
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
