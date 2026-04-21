import "wrappers/JettonMinter"

// Jetton contract imports
import "../contracts/errors"
import "../contracts/messages"
import "../contracts/storage"

// Acton library imports
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/message"

fun userWallet(minter: JettonMinter, userAddress: address): JettonWallet {
    val address = minter.getWalletAddress(userAddress);
    return JettonWallet { address };
}

fun setup_test(): (JettonMinter, Treasury, Treasury) {
    val deployer = testing.treasury("deployer");
    val notDeployer = testing.treasury("not_deployer");

    val minter = JettonMinter.fromStorage(MinterStorage {
        totalSupply: 1000000,
        adminAddress: deployer.address,
        content: createEmptyCell(),
        jettonWalletCode: build("jetton-wallet"),
    });

    val transactions = minter.sendDeploy(deployer.address, { value: ton("10") });
    expect(transactions).toHaveSuccessfulDeploy({ to: minter.address });

    return (minter, deployer, notDeployer);
}

get fun `test minter admin should be able to mint jettons`() {
    val (minter, deployer, notDeployer) = setup_test();
    val initialTotalSupply = minter.getTotalSupply();

    val wallet = userWallet(minter, deployer.address);
    val notDeployerWallet = userWallet(minter, notDeployer.address);

    val initialJettonBalance = ton("1000.23");
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        initialJettonBalance,
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({ from: minter.address, to: wallet.address });

    // excesses
    expect(mintResult).toHaveTx({
        from: wallet.address,
        to: minter.address,
        exitCode: 65535,
    });

    // check state
    expect(wallet.getJettonBalance()).toEqual(initialJettonBalance);
    expect(minter.getTotalSupply()).toEqual(initialTotalSupply + initialJettonBalance);

    val additionalJettonBalance = ton("2.31");
    val mintResult2 = minter.sendMint(
        deployer.address,
        deployer.address,
        additionalJettonBalance,
        ton("0.05"),
        ton("1"),
    );

    expect(mintResult2).toHaveSuccessfulTx({ from: minter.address, to: wallet.address });

    expect(mintResult2).toHaveTx({
        from: wallet.address,
        to: minter.address,
        exitCode: 65535,
    });

    expect(wallet.getJettonBalance()).toEqual(initialJettonBalance + additionalJettonBalance);
    expect(minter.getTotalSupply()).toEqual(
        initialTotalSupply + initialJettonBalance + additionalJettonBalance,
    );

    val otherJettonBalance = ton("3.12");
    val mintResult3 = minter.sendMint(
        deployer.address,
        notDeployer.address,
        otherJettonBalance,
        ton("0.05"),
        ton("1"),
    );

    expect(mintResult3).toHaveSuccessfulTx({ from: minter.address, to: notDeployerWallet.address });

    expect(mintResult3).toHaveTx({
        from: notDeployerWallet.address,
        to: minter.address,
        exitCode: 65535,
    });

    expect(notDeployerWallet.getJettonBalance()).toEqual(otherJettonBalance);
    expect(minter.getTotalSupply()).toEqual(
        initialTotalSupply + initialJettonBalance + additionalJettonBalance + otherJettonBalance,
    );
}

get fun `test not a minter admin should not be able to mint jettons`() {
    val (minter, deployer, notDeployer) = setup_test();
    val initialTotalSupply = minter.getTotalSupply();

    val wallet = userWallet(minter, deployer.address);
    val initialJettonBalance = wallet.getJettonBalance();

    val mintResult = minter.sendMint(
        notDeployer.address,
        deployer.address,
        ton("777"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveFailedTx({
        from: notDeployer.address,
        to: minter.address,
        exitCode: ERR_NOT_FROM_ADMIN,
    });

    expect(wallet.getJettonBalance()).toEqual(initialJettonBalance);
    expect(minter.getTotalSupply()).toEqual(initialTotalSupply);
}

get fun `test minter admin can change admin`() {
    val (minter, deployer, notDeployer) = setup_test();
    val adminBefore = minter.getAdminAddress();
    expect(adminBefore).toEqual(deployer.address);

    val changeAdminResult = minter.sendChangeAdmin(deployer.address, notDeployer.address);
    expect(changeAdminResult).toHaveSuccessfulTx({ from: deployer.address, to: minter.address });

    val adminAfter = minter.getAdminAddress();
    expect(adminAfter).toEqual(notDeployer.address);

    minter.sendChangeAdmin(notDeployer.address, deployer.address);
    expect(minter.getAdminAddress()).toEqual(deployer.address);
}

get fun `test not a minter admin can not change admin`() {
    val (minter, deployer, notDeployer) = setup_test();
    val adminBefore = minter.getAdminAddress();
    expect(adminBefore).toEqual(deployer.address);

    val changeAdminResult = minter.sendChangeAdmin(notDeployer.address, notDeployer.address);
    expect(changeAdminResult).toHaveFailedTx({
        from: notDeployer.address,
        to: minter.address,
        exitCode: ERR_NOT_FROM_ADMIN,
    });

    val adminAfter = minter.getAdminAddress();
    expect(adminAfter).toEqual(deployer.address);
}

get fun `test minter admin can change content`() {
    val (minter, deployer, _) = setup_test();

    // Get initial content
    val initialContent = minter.getContent();

    // Create new content
    val newContent = beginCell()
        .storeUint(1, 1)
        .storeSlice("https://totally_new_jetton.org/content.json".beginParse())
        .endCell();

    // Change content as admin
    val changeResult = minter.sendChangeContent(deployer.address, newContent);
    expect(changeResult).toHaveSuccessfulTx({ from: deployer.address, to: minter.address });

    // Check content changed
    expect(minter.getContent()).toEqual(newContent);

    // Change back to original content
    val changeBackResult = minter.sendChangeContent(deployer.address, initialContent);
    expect(changeBackResult).toHaveSuccessfulTx({ from: deployer.address, to: minter.address });

    // Check content is back to original
    expect(minter.getContent()).toEqual(initialContent);
}

get fun `test not a minter admin can not change content`() {
    val (minter, _, notDeployer) = setup_test();

    // Get initial content
    val initialContent = minter.getContent();

    // Try to change content as non-admin
    val newContent = beginCell().storeUint(1, 1).endCell();
    val changeResult = minter.sendChangeContent(notDeployer.address, newContent);

    expect(changeResult).toHaveFailedTx({
        from: notDeployer.address,
        to: minter.address,
        exitCode: ERR_NOT_FROM_ADMIN,
    });

    // Check content didn't change
    expect(minter.getContent()).toEqual(initialContent);
}

get fun `test wallet owner should be able to send jettons`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();
    val initialTotalSupply = minter.getTotalSupply();

    val notDeployerJettonWallet = userWallet(minter, notDeployer.address);
    val initialJettonBalance2 = notDeployerJettonWallet.getJettonBalance();

    val sentAmount = ton("0.5");
    val forwardAmount = ton("0.05");

    val sendResult = deployerJettonWallet.sendTransfer(
        deployer.address,
        ton("0.1"),
        sentAmount,
        notDeployer.address,
        deployer.address,
        null,
        forwardAmount,
        null,
    );

    expect(sendResult).toHaveSuccessfulTx({
        from: notDeployerJettonWallet.address,
        to: deployer.address,
    });                                                                                                        // excesses
    expect(sendResult).toHaveSuccessfulTx({
        from: notDeployerJettonWallet.address,
        to: notDeployer.address,
    });                                                                                                        // notification

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance - sentAmount);
    expect(notDeployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance2 + sentAmount);
    expect(minter.getTotalSupply()).toEqual(initialTotalSupply);
}

get fun `test not wallet owner should not be able to send jettons`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();
    val initialTotalSupply = minter.getTotalSupply();

    val notDeployerJettonWallet = userWallet(minter, notDeployer.address);
    val initialJettonBalance2 = notDeployerJettonWallet.getJettonBalance();

    val sentAmount = ton("0.5");

    val sendResult = deployerJettonWallet.sendTransfer(
        notDeployer.address,
        ton("0.1"),
        sentAmount,
        notDeployer.address,
        deployer.address,
        null,
        ton("0.05"),
        null,
    );

    expect(sendResult).toHaveFailedTx({
        from: notDeployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_NOT_FROM_OWNER,
    });

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance);
    expect(notDeployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance2);
    expect(minter.getTotalSupply()).toEqual(initialTotalSupply);
}

get fun `test impossible to send too much jettons`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();

    val notDeployerJettonWallet = userWallet(minter, notDeployer.address);
    val initialJettonBalance2 = notDeployerJettonWallet.getJettonBalance();

    val sentAmount = initialJettonBalance + 1;
    val forwardAmount = ton("0.05");

    val sendResult = deployerJettonWallet.sendTransfer(
        deployer.address,
        ton("0.1"),
        sentAmount,
        notDeployer.address,
        deployer.address,
        null,
        forwardAmount,
        null,
    );

    expect(sendResult).toHaveFailedTx({
        from: deployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_NOT_ENOUGH_BALANCE,
    });

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance);
    expect(notDeployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance2);
}

get fun `test malformed forward payload`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);

    val sentAmount = ton("0.5");
    val forwardAmount = random.range(ton("0.04")) + ton("0.01"); // 0.01-0.05

    val transferMsg = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: deployerJettonWallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: sentAmount,
            transferRecipient: notDeployer.address,
            sendExcessesTo: deployer.address,
            customPayload: null,
            forwardTonAmount: forwardAmount,
            forwardPayload: createEmptySlice(),
        },
    });

    val res = net.send(deployer.address, transferMsg);
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_INVALID_PAYLOAD,
    });
}

get fun `test correctly sends forward payload`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();

    val notDeployerJettonWallet = userWallet(minter, notDeployer.address);
    val initialJettonBalance2 = notDeployerJettonWallet.getJettonBalance();

    val sentAmount = ton("0.5");
    val forwardAmount = ton("0.05");
    val forwardPayload = beginCell().storeUint(0x1234567890abcdef, 128).endCell();

    val sendResult = deployerJettonWallet.sendTransfer(
        deployer.address,
        ton("0.1"),
        sentAmount,
        notDeployer.address,
        deployer.address,
        null,
        forwardAmount,
        forwardPayload,
    );

    expect(sendResult).toHaveSuccessfulTx({
        from: notDeployerJettonWallet.address,
        to: deployer.address,
    });                                                                                                     // excesses

    expect(sendResult).toHaveSuccessfulTx({
        from: notDeployerJettonWallet.address,
        to: notDeployer.address,
    });                                                                                                        // notification

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance - sentAmount);
    expect(notDeployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance2 + sentAmount);
}

get fun `test no forward ton amount no forward`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();

    val notDeployerJettonWallet = userWallet(minter, notDeployer.address);
    val initialJettonBalance2 = notDeployerJettonWallet.getJettonBalance();

    val sentAmount = ton("0.5");
    val forwardAmount = 0;
    val forwardPayload = beginCell().storeUint(0x1234567890abcdef, 128).endCell();

    val sendResult = deployerJettonWallet.sendTransfer(
        deployer.address,
        ton("0.1"),
        sentAmount,
        notDeployer.address,
        deployer.address,
        null,
        forwardAmount,
        forwardPayload,
    );

    expect(sendResult).toHaveSuccessfulTx({
        from: notDeployerJettonWallet.address,
        to: deployer.address,
    });                                                                                                     // excesses

    expect(sendResult).toNotHaveTx({
        from: notDeployerJettonWallet.address,
        to: notDeployer.address,
    });                                                                                                 // no notification

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance - sentAmount);
    expect(notDeployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance2 + sentAmount);
}

get fun `test check revert on not enough tons for forward`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();

    // Send 1 TON to deployer jetton wallet
    val tonTransferMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: deployerJettonWallet.address,
        body: createEmptySlice(),
    });
    net.send(deployer.address, tonTransferMsg);

    val sentAmount = ton("0.1");
    val forwardAmount = ton("0.3");
    val forwardPayload = beginCell().storeUint(0x1234567890abcdef, 128).endCell();

    val sendResult = deployerJettonWallet.sendTransfer(
        deployer.address,
        forwardAmount, // not enough tons, no tons for gas
        sentAmount,
        notDeployer.address,
        deployer.address,
        null,
        forwardAmount,
        forwardPayload,
    );

    expect(sendResult).toHaveFailedTx({
        from: deployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_NOT_ENOUGH_TON,
    });

    expect(sendResult).toHaveBouncedTx({
        from: deployerJettonWallet.address,
        to: deployer.address,
    });

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance);
}

get fun `test wallet does not accept internal transfer not from wallet`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();

    // Create InternalTransferStep message with invalid sender
    val internalTransferMsg = createMessage({
        bounce: false,
        value: ton("0.3"),
        dest: deployerJettonWallet.address,
        body: InternalTransferStep {
            queryId: 0,
            jettonAmount: ton("0.01"),
            transferInitiator: deployer.address,
            sendExcessesTo: deployer.address,
            forwardTonAmount: ton("0.05"),
            forwardPayload: beginCell().storeUint(0, 1).endCell().beginParse(),
        },
    });

    val sendResult = net.send(notDeployer.address, internalTransferMsg);

    expect(sendResult).toHaveFailedTx({
        from: notDeployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_INVALID_WALLET,
    });

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance);
}

get fun `test wallet owner should be able to burn jettons`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();
    val initialTotalSupply = minter.getTotalSupply();

    val burnAmount = ton("0.01");

    val sendResult = deployerJettonWallet.sendBurn(
        deployer.address,
        ton("0.1"), // ton amount
        burnAmount,
        deployer.address,
        null,       // custom payload
    );

    expect(sendResult).toHaveSuccessfulTx({
        from: deployerJettonWallet.address,
        to: minter.address,
    });                                                                                                // burn notification

    expect(sendResult).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
    });                                                                                    // excesses

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance - burnAmount);
    expect(minter.getTotalSupply()).toEqual(initialTotalSupply - burnAmount);
}

get fun `test not wallet owner should not be able to burn jettons`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();
    val initialTotalSupply = minter.getTotalSupply();

    val burnAmount = ton("0.01");

    val sendResult = deployerJettonWallet.sendBurn(
        notDeployer.address,
        ton("0.1"), // ton amount
        burnAmount,
        deployer.address,
        null,       // custom payload
    );

    expect(sendResult).toHaveFailedTx({
        from: notDeployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_NOT_FROM_OWNER,
    });

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance);
    expect(minter.getTotalSupply()).toEqual(initialTotalSupply);
}

get fun `test wallet owner can not burn more jettons than it has`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val initialJettonBalance = deployerJettonWallet.getJettonBalance();
    val initialTotalSupply = minter.getTotalSupply();

    val burnAmount = initialJettonBalance + 1;

    val sendResult = deployerJettonWallet.sendBurn(
        deployer.address,
        ton("0.1"), // ton amount
        burnAmount,
        deployer.address,
        null,       // custom payload
    );

    expect(sendResult).toHaveFailedTx({
        from: deployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_NOT_ENOUGH_BALANCE,
    });

    expect(deployerJettonWallet.getJettonBalance()).toEqual(initialJettonBalance);
    expect(minter.getTotalSupply()).toEqual(initialTotalSupply);
}

get fun `test minter should only accept burn messages from jetton wallets`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val burnAmount = ton("1");

    // Create burn notification with wrong wallet address
    val burnNotificationWrong = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: minter.address,
        body: BurnNotificationForMinter {
            queryId: 0,
            jettonAmount: burnAmount,
            burnInitiator: randomAddress("wrong-address"), // wrong address
            sendExcessesTo: deployer.address,
        },
    });

    val sendResultWrong = net.send(deployerJettonWallet.address, burnNotificationWrong);

    expect(sendResultWrong).toHaveFailedTx({
        from: deployerJettonWallet.address,
        to: minter.address,
        exitCode: ERR_UNAUTHORIZED_BURN,
    });

    // Create burn notification with correct wallet address
    val burnNotificationCorrect = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: minter.address,
        body: BurnNotificationForMinter {
            queryId: 0,
            jettonAmount: burnAmount,
            burnInitiator: deployer.address,
            sendExcessesTo: deployer.address,
        },
    });

    val sendResultCorrect = net.send(deployerJettonWallet.address, burnNotificationCorrect);

    expect(sendResultCorrect).toHaveSuccessfulTx({
        from: deployerJettonWallet.address,
        to: minter.address,
    });
}

get fun `test report correct discovery address`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Test discovery with includeOwnerAddress = true for deployer
    val discoveryResultDeployer = minter.sendDiscovery(deployer.address, deployer.address, true);
    val deployerJettonWallet = userWallet(minter, deployer.address);

    expect(discoveryResultDeployer).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: deployerJettonWallet.address,
            ownerAddress: deployer.address.toCell(),
        }.toCell(),
    });

    // Test discovery with includeOwnerAddress = true for notDeployer
    val discoveryResultNotDeployer = minter.sendDiscovery(
        deployer.address,
        notDeployer.address,
        true,
    );
    val notDeployerJettonWallet = userWallet(minter, notDeployer.address);

    expect(discoveryResultNotDeployer).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: notDeployerJettonWallet.address,
            ownerAddress: notDeployer.address.toCell(),
        }.toCell(),
    });

    // Test discovery with includeOwnerAddress = false
    val discoveryResultNoInclude = minter.sendDiscovery(
        deployer.address,
        notDeployer.address,
        false,
    );

    expect(discoveryResultNoInclude).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: notDeployerJettonWallet.address,
            ownerAddress: null,
        }.toCell(),
    });
}

get fun `test fail to report discovery address with not enough tons`() {
    val (minter, deployer, _) = setup_test();

    val discoveryRes = minter.sendDiscovery(deployer.address, deployer.address, true, ton("0.01"));
    expect(discoveryRes).toHaveFailedTx({
        from: deployer.address,
        to: minter.address,
        exitCode: ERR_NOT_ENOUGH_AMOUNT_TO_RESPOND,
    });
}

get fun `test wallet should restore balance on internal transfer bounce`() {
    val (minter, deployer, notDeployer) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("2"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val notDeployerJettonWallet = userWallet(minter, notDeployer.address);

    val balanceBefore = deployerJettonWallet.getJettonBalance();
    val txAmount = random.range(100) + 100; // random amount between 100-200

    // Send transfer message
    val transferMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: deployerJettonWallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: txAmount,
            transferRecipient: notDeployer.address,
            sendExcessesTo: deployer.address,
            customPayload: null,
            forwardTonAmount: 0,
            forwardPayload: beginCell().storeMaybeRef(null).endCell().beginParse(),
        },
    });

    val transferResult = testing.processSingleTraceStep(deployer.address, transferMsg);
    expect(transferResult.outMessages.size()).toEqual(1);
    val outMsgBody = transferResult.outMessages.at<InternalTransferStep>(0).loadBody();

    // Check balance decreased
    expect(deployerJettonWallet.getJettonBalance()).toEqual(balanceBefore - txAmount);

    // Send bounced message back
    val bouncedMsg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: deployerJettonWallet.address,
        body: beginCell()
            .storeUint(0xFFFFFFFF, 32)
            .storeSlice(outMsgBody.toCell().beginParse())
            .endCell(),
    }).bounced();

    testing.processSingleTraceStep(notDeployerJettonWallet.address, bouncedMsg);

    // Balance should be restored
    expect(deployerJettonWallet.getJettonBalance()).toEqual(balanceBefore);
}

get fun `test wallet should restore balance on burn notification bounce`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("2"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val balanceBefore = deployerJettonWallet.getJettonBalance();
    val burnAmount = random.range(100) + 100; // random amount between 100-200

    // Send burn message
    val burnMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: deployerJettonWallet.address,
        body: AskToBurn {
            queryId: 0,
            jettonAmount: burnAmount,
            sendExcessesTo: deployer.address,
            customPayload: null,
        },
    });

    val burnResult = testing.processSingleTraceStep(deployer.address, burnMsg);
    expect(burnResult.outMessages.size()).toEqual(1);
    val outMsgBody = burnResult.outMessages.at<BurnNotificationForMinter>(0).loadBody();

    // Check balance decreased
    expect(deployerJettonWallet.getJettonBalance()).toEqual(balanceBefore - burnAmount);

    // Send bounced message back from minter
    val bouncedMsg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: deployerJettonWallet.address,
        body: beginCell()
            .storeUint(0xFFFFFFFF, 32)
            .storeSlice(outMsgBody.toCell().beginParse())
            .endCell(),
    }).bounced();

    testing.processSingleTraceStep(minter.address, bouncedMsg);

    // Balance should be restored
    expect(deployerJettonWallet.getJettonBalance()).toEqual(balanceBefore);
}

get fun `test correctly handles not valid address in discovery`() {
    val (minter, deployer, _) = setup_test();

    // Create a bad address (workchain -1)
    val badAddress = randomAddress("bad_addr", -1);

    // Test discovery with bad address and includeOwnerAddress = false
    val discoveryResultBadNoInclude = minter.sendDiscovery(deployer.address, badAddress, false);

    expect(discoveryResultBadNoInclude).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: null,
            ownerAddress: null,
        }.toCell(),
    });

    // Test discovery with bad address and includeOwnerAddress = true
    val discoveryResultBadInclude = minter.sendDiscovery(deployer.address, badAddress, true);

    expect(discoveryResultBadInclude).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: null,
            ownerAddress: badAddress.toCell(),
        }.toCell(),
    });
}

get fun `test can not send to masterchain`() {
    val (minter, deployer, _) = setup_test();

    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);
    val sentAmount = ton("0.5");
    val forwardAmount = ton("0.05");

    // Create masterchain address (workchain -1)
    val masterchainAddress = address("Ef8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADAU");

    val sendResult = deployerJettonWallet.sendTransfer(
        deployer.address,
        ton("0.1"), // tons
        sentAmount,
        masterchainAddress,
        deployer.address,
        null,
        forwardAmount,
        null,
    );

    expect(sendResult).toHaveFailedTx({
        from: deployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_WRONG_WORKCHAIN,
    });
}

get fun `test transfer to masterchain`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1"),
    );
    expect(mintResult).toHaveSuccessfulTx({
        from: minter.address,
        to: userWallet(minter, deployer.address).address,
    });

    val deployerJettonWallet = userWallet(minter, deployer.address);

    val invalidAddress = randomAddress("invalid", -1);

    val transferMsg = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: deployerJettonWallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: ton("0.5"),
            transferRecipient: invalidAddress,
            sendExcessesTo: deployer.address,
            customPayload: null,
            forwardTonAmount: 0,
            forwardPayload: beginCell().storeUint(0, 1).endCell().beginParse(),
        },
    });

    val res = net.send(deployer.address, transferMsg);
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: deployerJettonWallet.address,
        exitCode: ERR_WRONG_WORKCHAIN,
    });
}
