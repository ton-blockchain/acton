// Acton library imports
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/transaction"
import "../../lib/types/message"

// Jetton contract imports
import "../contracts/errors"
import "../contracts/messages"
import "../contracts/storage"

import "wrappers/JettonMinter"

fun userWallet(minter: JettonMinter, userAddress: address): JettonWallet {
    val address = minter.getWalletAddress(userAddress);
    return JettonWallet { address }
}

fun setup_test(): (JettonMinter, Treasury, Treasury) {
    val deployer = net.treasury("deployer");
    val not_deployer = net.treasury("not_deployer");

    val minter = JettonMinter.fromStorage(MinterStorage {
        totalSupply: 1000000,
        adminAddress: deployer.address,
        content: createEmptyCell(),
        jettonWalletCode: build("jetton-wallet"),
    });

    val transactions = minter.sendDeploy({ value: ton("10") });
    expect(transactions).toHaveSuccessfulDeploy({ to: minter.address });

    return (minter, deployer, not_deployer);
}

get fun `test minter admin should be able to mint jettons`() {
    val (minter, deployer, not_deployer) = setup_test();
    val initial_total_supply = minter.getTotalSupply();

    val wallet = userWallet(minter, deployer.address);
    val not_deployer_wallet = userWallet(minter, not_deployer.address);

    val initial_jetton_balance = ton("1000.23");
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        initial_jetton_balance,
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: wallet.address });

    // excesses
    expect(mint_result).toHaveTx({ from: wallet.address, to: minter.address, exitCode: 65535 });

    // check state
    expect(wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply + initial_jetton_balance);

    val additional_jetton_balance = ton("2.31");
    val mint_result_2 = minter.sendMint(
        deployer.address,
        deployer.address,
        additional_jetton_balance,
        ton("0.05"),
        ton("1")
    );

    expect(mint_result_2).toHaveSuccessfulTx({ from: minter.address, to:  wallet.address });

    expect(mint_result_2).toHaveTx({
        from:  wallet.address,
        to: minter.address,
        exitCode: 65535
    });

    expect( wallet.getJettonBalance()).toEqual(initial_jetton_balance + additional_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply + initial_jetton_balance + additional_jetton_balance);

    val other_jetton_balance = ton("3.12");
    val mint_result_3 = minter.sendMint(
        deployer.address,
        not_deployer.address,
        other_jetton_balance,
        ton("0.05"),
        ton("1")
    );

    expect(mint_result_3).toHaveSuccessfulTx({ from: minter.address, to: not_deployer_wallet.address });

    expect(mint_result_3).toHaveTx({
        from: not_deployer_wallet.address,
        to: minter.address,
        exitCode: 65535
    });

    expect(not_deployer_wallet.getJettonBalance()).toEqual(other_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(
        initial_total_supply + initial_jetton_balance + additional_jetton_balance + other_jetton_balance
    );
}

get fun `test not a minter admin should not be able to mint jettons`() {
    val (minter, deployer, not_deployer) = setup_test();
    val initial_total_supply = minter.getTotalSupply();

    val wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = wallet.getJettonBalance();

    val mint_result = minter.sendMint(
        not_deployer.address,
        deployer.address,
        ton("777"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveFailedTx({ from: not_deployer.address, to: minter.address, exitCode: ERR_NOT_FROM_ADMIN });

    expect(wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test minter admin can change admin`() {
    val (minter, deployer, not_deployer) = setup_test();
    val admin_before = minter.getAdminAddress();
    expect(admin_before).toEqual(deployer.address);

    val changeAdminResult = minter.sendChangeAdmin(deployer.address, not_deployer.address);
    expect(changeAdminResult).toHaveSuccessfulTx({ from: deployer.address, to: minter.address });

    val admin_after = minter.getAdminAddress();
    expect(admin_after).toEqual(not_deployer.address);

    minter.sendChangeAdmin(not_deployer.address, deployer.address);
    expect(minter.getAdminAddress()).toEqual(deployer.address);
}

get fun `test not a minter admin can not change admin`() {
    val (minter, deployer, not_deployer) = setup_test();
    val admin_before = minter.getAdminAddress();
    expect(admin_before).toEqual(deployer.address);

    val changeAdminResult = minter.sendChangeAdmin(not_deployer.address, not_deployer.address);
    expect(changeAdminResult).toHaveFailedTx({ from: not_deployer.address, to: minter.address, exitCode: ERR_NOT_FROM_ADMIN });

    val admin_after = minter.getAdminAddress();
    expect(admin_after).toEqual(deployer.address);
}

get fun `test minter admin can change content`() {
    val (minter, deployer, _) = setup_test();

    // Get initial content
    val initial_content = minter.getContent();

    // Create new content
    val new_content = beginCell().storeUint(1, 1).storeSlice("https://totally_new_jetton.org/content.json".beginParse()).endCell();

    // Change content as admin
    val change_result = minter.sendChangeContent(deployer.address, new_content);
    expect(change_result).toHaveSuccessfulTx({
        from: deployer.address,
        to: minter.address
    });

    // Check content changed
    expect(minter.getContent()).toEqual(new_content);

    // Change back to original content
    val change_back_result = minter.sendChangeContent(deployer.address, initial_content);
    expect(change_back_result).toHaveSuccessfulTx({
        from: deployer.address,
        to: minter.address
    });

    // Check content is back to original
    expect(minter.getContent()).toEqual(initial_content);
}

get fun `test not a minter admin can not change content`() {
    val (minter, _, not_deployer) = setup_test();

    // Get initial content
    val initial_content = minter.getContent();

    // Try to change content as non-admin
    val new_content = beginCell().storeUint(1, 1).endCell();
    val change_result = minter.sendChangeContent(not_deployer.address, new_content);

    expect(change_result).toHaveFailedTx({
        from: not_deployer.address,
        to: minter.address,
        exitCode: ERR_NOT_FROM_ADMIN
    });

    // Check content didn't change
    expect(minter.getContent()).toEqual(initial_content);
}

get fun `test wallet owner should be able to send jettons`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();
    val initial_total_supply = minter.getTotalSupply();

    val not_deployer_jetton_wallet = userWallet(minter, not_deployer.address);
    val initial_jetton_balance_2 = not_deployer_jetton_wallet.getJettonBalance();

    val sent_amount = ton("0.5");
    val forward_amount = ton("0.05");

    val send_result = deployer_jetton_wallet.sendTransfer(
        deployer.address,
        ton("0.1"),
        sent_amount,
        not_deployer.address,
        deployer.address,
        null,
        forward_amount,
        null
    );

    expect(send_result).toHaveSuccessfulTx({ from: not_deployer_jetton_wallet.address, to: deployer.address }); // excesses
    expect(send_result).toHaveSuccessfulTx({ from: not_deployer_jetton_wallet.address, to: not_deployer.address }); // notification

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance - sent_amount);
    expect(not_deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance_2 + sent_amount);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test not wallet owner should not be able to send jettons`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();
    val initial_total_supply = minter.getTotalSupply();

    val not_deployer_jetton_wallet = userWallet(minter, not_deployer.address);
    val initial_jetton_balance_2 = not_deployer_jetton_wallet.getJettonBalance();

    val sent_amount = ton("0.5");

    val send_result = deployer_jetton_wallet.sendTransfer(
        not_deployer.address,
        ton("0.1"),
        sent_amount,
        not_deployer.address,
        deployer.address,
        null,
        ton("0.05"),
        null
    );

    expect(send_result).toHaveFailedTx({
        from: not_deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_NOT_FROM_OWNER
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(not_deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance_2);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test impossible to send too much jettons`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();

    val not_deployer_jetton_wallet = userWallet(minter, not_deployer.address);
    val initial_jetton_balance_2 = not_deployer_jetton_wallet.getJettonBalance();

    val sent_amount = initial_jetton_balance + 1;
    val forward_amount = ton("0.05");

    val send_result = deployer_jetton_wallet.sendTransfer(
        deployer.address,
        ton("0.1"),
        sent_amount,
        not_deployer.address,
        deployer.address,
        null,
        forward_amount,
        null
    );

    expect(send_result).toHaveFailedTx({
        from: deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_NOT_ENOUGH_BALANCE
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(not_deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance_2);
}

get fun `test malformed forward payload`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);

    val sent_amount = ton("0.5");
    val forward_amount = random.range(ton("0.04")) + ton("0.01"); // 0.01-0.05

    val transfer_msg = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: deployer_jetton_wallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: sent_amount,
            transferRecipient: not_deployer.address,
            sendExcessesTo: deployer.address,
            customPayload: null,
            forwardTonAmount: forward_amount,
            forwardPayload: createEmptySlice(),
        }.toCell(),
    });

    val res = net.send(deployer.address, transfer_msg, SEND_MODE_REGULAR);
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_INVALID_PAYLOAD,
    });
}

get fun `test correctly sends forward payload`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();

    val not_deployer_jetton_wallet = userWallet(minter, not_deployer.address);
    val initial_jetton_balance_2 = not_deployer_jetton_wallet.getJettonBalance();

    val sent_amount = ton("0.5");
    val forward_amount = ton("0.05");
    val forward_payload = beginCell().storeUint(0x1234567890abcdef, 128).endCell();

    val send_result = deployer_jetton_wallet.sendTransfer(
        deployer.address,
        ton("0.1"),
        sent_amount,
        not_deployer.address,
        deployer.address,
        null,
        forward_amount,
        forward_payload
    );

    expect(send_result).toHaveSuccessfulTx({
        from: not_deployer_jetton_wallet.address,
        to: deployer.address
    }); // excesses

    expect(send_result).toHaveSuccessfulTx({
        from: not_deployer_jetton_wallet.address,
        to: not_deployer.address
    }); // notification

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance - sent_amount);
    expect(not_deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance_2 + sent_amount);
}

get fun `test no forward ton amount no forward`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();

    val not_deployer_jetton_wallet = userWallet(minter, not_deployer.address);
    val initial_jetton_balance_2 = not_deployer_jetton_wallet.getJettonBalance();

    val sent_amount = ton("0.5");
    val forward_amount = 0;
    val forward_payload = beginCell().storeUint(0x1234567890abcdef, 128).endCell();

    val send_result = deployer_jetton_wallet.sendTransfer(
        deployer.address,
        ton("0.1"),
        sent_amount,
        not_deployer.address,
        deployer.address,
        null,
        forward_amount,
        forward_payload
    );

    expect(send_result).toHaveSuccessfulTx({
        from: not_deployer_jetton_wallet.address,
        to: deployer.address
    }); // excesses

    expect(send_result).toNotHaveTx({
        from: not_deployer_jetton_wallet.address,
        to: not_deployer.address
    }); // no notification

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance - sent_amount);
    expect(not_deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance_2 + sent_amount);
}

get fun `test check revert on not enough tons for forward`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();

    // Send 1 TON to deployer jetton wallet
    val ton_transfer_msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: deployer_jetton_wallet.address,
        body: createEmptySlice(),
    });
    net.send(deployer.address, ton_transfer_msg, SEND_MODE_PAY_FEES_SEPARATELY);

    val sent_amount = ton("0.1");
    val forward_amount = ton("0.3");
    val forward_payload = beginCell().storeUint(0x1234567890abcdef, 128).endCell();

    val send_result = deployer_jetton_wallet.sendTransfer(
        deployer.address,
        forward_amount, // not enough tons, no tons for gas
        sent_amount,
        not_deployer.address,
        deployer.address,
        null,
        forward_amount,
        forward_payload
    );

    expect(send_result).toHaveFailedTx({
        from: deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_NOT_ENOUGH_TON
    });

    expect(send_result).toHaveBouncedTx({
        from: deployer_jetton_wallet.address,
        to: deployer.address,
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
}

get fun `test wallet does not accept internal transfer not from wallet`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();

    // Create InternalTransferStep message with invalid sender
    val internal_transfer_msg = createMessage({
        bounce: false,
        value: ton("0.3"),
        dest: deployer_jetton_wallet.address,
        body: InternalTransferStep {
            queryId: 0,
            jettonAmount: ton("0.01"),
            transferInitiator: deployer.address,
            sendExcessesTo: deployer.address,
            forwardTonAmount: ton("0.05"),
            forwardPayload: beginCell().storeUint(0, 1).endCell().beginParse(),
        }
    });

    val send_result = net.send(not_deployer.address, internal_transfer_msg, SEND_MODE_PAY_FEES_SEPARATELY);

    expect(send_result).toHaveFailedTx({
        from: not_deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_INVALID_WALLET
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
}

get fun `test wallet owner should be able to burn jettons`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();
    val initial_total_supply = minter.getTotalSupply();

    val burn_amount = ton("0.01");

    val send_result = deployer_jetton_wallet.sendBurn(
        deployer.address,
        ton("0.1"), // ton amount
        burn_amount,
        deployer.address,
        null // custom payload
    );

    expect(send_result).toHaveSuccessfulTx({
        from: deployer_jetton_wallet.address,
        to: minter.address
    }); // burn notification

    expect(send_result).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address
    }); // excesses

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance - burn_amount);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply - burn_amount);
}

get fun `test not wallet owner should not be able to burn jettons`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();
    val initial_total_supply = minter.getTotalSupply();

    val burn_amount = ton("0.01");

    val send_result = deployer_jetton_wallet.sendBurn(
        not_deployer.address,
        ton("0.1"), // ton amount
        burn_amount,
        deployer.address,
        null // custom payload
    );

    expect(send_result).toHaveFailedTx({
        from: not_deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_NOT_FROM_OWNER
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test wallet owner can not burn more jettons than it has`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val initial_jetton_balance = deployer_jetton_wallet.getJettonBalance();
    val initial_total_supply = minter.getTotalSupply();

    val burn_amount = initial_jetton_balance + 1;

    val send_result = deployer_jetton_wallet.sendBurn(
        deployer.address,
        ton("0.1"), // ton amount
        burn_amount,
        deployer.address,
        null // custom payload
    );

    expect(send_result).toHaveFailedTx({
        from: deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_NOT_ENOUGH_BALANCE
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test minter should only accept burn messages from jetton wallets`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val burn_amount = ton("1");

    // Create burn notification with wrong wallet address
    val burn_notification_wrong = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: minter.address,
        body: BurnNotificationForMinter {
            queryId: 0,
            jettonAmount: burn_amount,
            burnInitiator: net.randomAddress("wrong-address"), // wrong address
            sendExcessesTo: deployer.address,
        }
    });

    val send_result_wrong = net.send(deployer_jetton_wallet.address, burn_notification_wrong, SEND_MODE_PAY_FEES_SEPARATELY);

    expect(send_result_wrong).toHaveFailedTx({
        from: deployer_jetton_wallet.address,
        to: minter.address,
        exitCode: ERR_UNAUTHORIZED_BURN
    });

    // Create burn notification with correct wallet address
    val burn_notification_correct = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: minter.address,
        body: BurnNotificationForMinter {
            queryId: 0,
            jettonAmount: burn_amount,
            burnInitiator: deployer.address,
            sendExcessesTo: deployer.address,
        }
    });

    val send_result_correct = net.send(deployer_jetton_wallet.address, burn_notification_correct, SEND_MODE_PAY_FEES_SEPARATELY);

    expect(send_result_correct).toHaveSuccessfulTx({
        from: deployer_jetton_wallet.address,
        to: minter.address
    });
}

get fun `test report correct discovery address`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Test discovery with includeOwnerAddress = true for deployer
    val discovery_result_deployer = minter.sendDiscovery(deployer.address, deployer.address, true);
    val deployer_jetton_wallet = userWallet(minter, deployer.address);

    expect(discovery_result_deployer).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: deployer_jetton_wallet.address,
            ownerAddress: deployer.address.toCell(),
        }.toCell(),
    });

    // Test discovery with includeOwnerAddress = true for notDeployer
    val discovery_result_not_deployer = minter.sendDiscovery(deployer.address, not_deployer.address, true);
    val not_deployer_jetton_wallet = userWallet(minter, not_deployer.address);

    expect(discovery_result_not_deployer).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: not_deployer_jetton_wallet.address,
            ownerAddress: not_deployer.address.toCell(),
        }.toCell(),
    });

    // Test discovery with includeOwnerAddress = false
    val discovery_result_no_include = minter.sendDiscovery(deployer.address, not_deployer.address, false);

    expect(discovery_result_no_include).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: not_deployer_jetton_wallet.address,
            ownerAddress: null,
        }.toCell(),
    });
}

get fun `test fail to report discovery address with not enough tons`() {
    val (minter, deployer, _) = setup_test();

    val discovery_res = minter.sendDiscovery(deployer.address, deployer.address, true, ton("0.01"));
    expect(discovery_res).toHaveFailedTx({
        from: deployer.address,
        to: minter.address,
        exitCode: ERR_NOT_ENOUGH_AMOUNT_TO_RESPOND,
    });
}

get fun `test wallet should restore balance on internal transfer bounce`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("2"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val not_deployer_jetton_wallet = userWallet(minter, not_deployer.address);

    val balance_before = deployer_jetton_wallet.getJettonBalance();
    val tx_amount = random.range(100) + 100; // random amount between 100-200

    // Send transfer message
    val transfer_msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: deployer_jetton_wallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: tx_amount,
            transferRecipient: not_deployer.address,
            sendExcessesTo: deployer.address,
            customPayload: null,
            forwardTonAmount: 0,
            forwardPayload: beginCell().storeMaybeRef(null).endCell().beginParse(),
        }.toCell(),
    });

    val transfer_result = net.sendSingle(deployer.address, transfer_msg);
    expect(transfer_result.outMessages.size()).toEqual(1);
    val out_msg_body = transfer_result.outMessages.at<InternalTransferStep>(0).loadBody();

    // Check balance decreased
    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(balance_before - tx_amount);

    // Send bounced message back
    val bounced_msg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: deployer_jetton_wallet.address,
        body: beginCell().storeUint(0xFFFFFFFF, 32).storeSlice(out_msg_body.toCell().beginParse()).endCell(),
    }).bounced();

    net.sendSingle(not_deployer_jetton_wallet.address, bounced_msg);

    // Balance should be restored
    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(balance_before);
}

get fun `test wallet should restore balance on burn notification bounce`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("2"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val balance_before = deployer_jetton_wallet.getJettonBalance();
    val burn_amount = random.range(100) + 100; // random amount between 100-200

    // Send burn message
    val burn_msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: deployer_jetton_wallet.address,
        body: AskToBurn {
            queryId: 0,
            jettonAmount: burn_amount,
            sendExcessesTo: deployer.address,
            customPayload: null,
        }.toCell(),
    });

    val burn_result = net.sendSingle(deployer.address, burn_msg);
    expect(burn_result.outMessages.size()).toEqual(1);
    val out_msg_body = burn_result.outMessages.at<BurnNotificationForMinter>(0).loadBody();

    // Check balance decreased
    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(balance_before - burn_amount);

    // Send bounced message back from minter
    val bounced_msg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: deployer_jetton_wallet.address,
        body: beginCell().storeUint(0xFFFFFFFF, 32).storeSlice(out_msg_body.toCell().beginParse()).endCell(),
    }).bounced();

    net.sendSingle(minter.address, bounced_msg);

    // Balance should be restored
    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(balance_before);
}

get fun `test correctly handles not valid address in discovery`() {
    val (minter, deployer, _) = setup_test();

    // Create a bad address (workchain -1)
    val bad_address = net.randomAddress("bad_addr", -1);;

    // Test discovery with bad address and includeOwnerAddress = false
    val discovery_result_bad_no_include = minter.sendDiscovery(deployer.address, bad_address, false);

    expect(discovery_result_bad_no_include).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: null,
            ownerAddress: null,
        }.toCell(),
    });

    // Test discovery with bad address and includeOwnerAddress = true
    val discovery_result_bad_include = minter.sendDiscovery(deployer.address, bad_address, true);

    expect(discovery_result_bad_include).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address,
        body: ResponseWalletAddress {
            queryId: 0,
            jettonWalletAddress: null,
            ownerAddress: bad_address.toCell(),
        }.toCell(),
    });
}

get fun `test can not send to masterchain`() {
    val (minter, deployer, _) = setup_test();

    val deployer_jetton_wallet = userWallet(minter, deployer.address);
    val sent_amount = ton("0.5");
    val forward_amount = ton("0.05");

    // Create masterchain address (workchain -1)
    val masterchain_address = address("Ef8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADAU");

    val send_result = deployer_jetton_wallet.sendTransfer(
        deployer.address,
        ton("0.1"), // tons
        sent_amount,
        masterchain_address,
        deployer.address,
        null,
        forward_amount,
        null
    );

    expect(send_result).toHaveFailedTx({
        from: deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_WRONG_WORKCHAIN
    });
}

get fun `test transfer to masterchain`() {
    val (minter, deployer, _) = setup_test();

    // Mint some jettons to deployer first
    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("1"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

    val deployer_jetton_wallet = userWallet(minter, deployer.address);

    val invalid_address = net.randomAddress("invalid", -1);

    val transfer_msg = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: deployer_jetton_wallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: ton("0.5"),
            transferRecipient: invalid_address,
            sendExcessesTo: deployer.address,
            customPayload: null,
            forwardTonAmount: 0,
            forwardPayload: beginCell().storeUint(0, 1).endCell().beginParse(),
        }.toCell(),
    });

    val res = net.send(deployer.address, transfer_msg, SEND_MODE_REGULAR);
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: deployer_jetton_wallet.address,
        exitCode: ERR_WRONG_WORKCHAIN,
    });
}
