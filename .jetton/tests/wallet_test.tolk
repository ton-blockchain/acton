// Acton library imports
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/transaction"

// Jetton contract imports
import "../contracts/errors"
import "../contracts/messages"
import "../contracts/storage"

struct JettonWallet {
    address: address
}

fun JettonWallet.getJettonBalance(self): coins {
    if (!net.isDeployed(self.address)) {
        return 0
    }

    val result: JettonWalletDataReply = net.runGetMethod(self.address, "get_wallet_data");
    return result.jettonBalance
}

fun JettonWallet.sendTransfer(self, from: address, value: int, jettonAmount: int, recepient: address, responseAddress: address, customPayload: cell?, forwardTonAmount: int, forwardPayload: cell?): TransactionList {
    val msg = createMessage({
        bounce: true,
        value: value,
        dest: self.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: jettonAmount,
            transferRecipient: recepient,
            sendExcessesTo: responseAddress,
            customPayload: customPayload,
            forwardTonAmount: forwardTonAmount,
            forwardPayload: beginCell().storeMaybeRef(forwardPayload.toCell()).endCell().beginParse(),
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonWallet.sendBurn(self, from: address, value: int, jettonAmount: int, responseAddress: address, customPayload: cell?): TransactionList {
    val msg = createMessage({
        bounce: false,
        value: value,
        dest: self.address,
        body: AskToBurn {
            queryId: 0,
            jettonAmount: jettonAmount,
            sendExcessesTo: responseAddress,
            customPayload: customPayload,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.sendDiscovery(self, from: address, ownerAddress: address, includeOwnerAddress: bool): TransactionList {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: RequestWalletAddress {
            queryId: 0,
            ownerAddress: ownerAddress,
            includeOwnerAddress: includeOwnerAddress,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.sendChangeContent(self, from: address, newContent: cell): TransactionList {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: ChangeMinterContent {
            queryId: 0,
            newContent: newContent,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

struct JettonMinter {
    address: address
    init: ContractState
}

fun JettonMinter.fromStorage(storage: MinterStorage) {
    val init = ContractState {
        // TODO: in the future version there will be no path at all
        code: build("Minter", "./.jetton/contracts/jetton-minter-contract.tolk"),
        data: storage.toCell(),
    };
    val addr = address.fromValidBuilder(AutoDeployAddress { stateInit: init }.buildAddress());
    return JettonMinter {
        address: addr, init
    }
}

fun JettonMinter.sendDeploy(self, params: SendParams) {
    val msg = createMessage({
        bounce: false,
        value: params.value,
        dest: {
            workchain: 0,
            stateInit: self.init,
        },
    });
    return net.send(net.treasury("trasury").address, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.sendMint(self, from: address, recipient: address, jetton_amount: int, forward_ton_amount: int, total_ton_amount: int) {
    val msg = createMessage({
        bounce: false,
        value: total_ton_amount + ton("0.015"),
        dest: self.address,
        body: MintNewJettons {
            queryId: 0,
            mintRecipient: recipient,
            tonAmount: total_ton_amount,
            internalTransferMsg: InternalTransferStep {
                queryId: 0,
                jettonAmount: jetton_amount,
                transferInitiator: createAddressNone(),
                sendExcessesTo: self.address,
                forwardTonAmount: forward_ton_amount,
                forwardPayload: createEmptySlice(),
            }.toCell(),
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.sendChangeAdmin(self, from: address, new_owner: address) {
    val msg = createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: self.address,
        body: ChangeMinterAdmin {
            queryId: 0,
            newAdminAddress: new_owner,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.getJettonData(self): JettonDataReply {
    return net.runGetMethod(self.address, "get_jetton_data");
}

fun JettonMinter.getTotalSupply(self): int {
    return self.getJettonData().totalSupply
}

fun JettonMinter.getAdminAddress(self): address {
    return self.getJettonData().adminAddress
}

fun JettonMinter.getContent(self): cell {
    return self.getJettonData().jettonContent
}

fun JettonMinter.getWalletAddress(self, owner: address): address {
    return net.runGetMethod(self.address, "get_wallet_address", owner)
}

fun userWallet(minter: JettonMinter, userAddress: address): JettonWallet {
    val address = minter.getWalletAddress(userAddress);
    return JettonWallet { address }
}

fun setup_test(): (JettonMinter, Treasury, Treasury) {
    val deployer = net.treasury("deployer");

    val minter = JettonMinter.fromStorage(MinterStorage {
        totalSupply: 1000000,
        adminAddress: deployer.address,
        content: createEmptyCell(),
        jettonWalletCode: build("Wallet", "./.jetton/contracts/jetton-wallet-contract.tolk"),
    });

    val transactions = minter.sendDeploy({ value: ton("10") });
    expect(transactions).toHaveSuccessfulDeploy({ to: minter.address });

    val not_deployer = net.treasury("not_deployer");

    return (minter, deployer, not_deployer);
}

get fun `test-can-mint`() {
    val (minter, deployer, _) = setup_test();
    val initial_total_supply = minter.getTotalSupply();

    val wallet = userWallet(minter, deployer.address);

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
    expect(mint_result).toHaveSuccessfulTx({ from: wallet.address, to: minter.address, exit_code: 65535 });

    // check state
    expect(wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply + initial_jetton_balance)
}

get fun `test-not-a-minter-admin-should-not-be-able-to-mint-jettons`() {
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
    expect(mint_result).toHaveFailedTx({ from: not_deployer.address, to: minter.address, exit_code: ERR_NOT_FROM_ADMIN });

    expect(wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test-minter-admin-can-change-admin`() {
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

get fun `test-not-a-minter-admin-can-not-change-admin`() {
    val (minter, deployer, not_deployer) = setup_test();
    val admin_before = minter.getAdminAddress();
    expect(admin_before).toEqual(deployer.address);

    val changeAdminResult = minter.sendChangeAdmin(not_deployer.address, not_deployer.address);
    expect(changeAdminResult).toHaveFailedTx({ from: not_deployer.address, to: minter.address, exit_code: ERR_NOT_FROM_ADMIN });

    val admin_after = minter.getAdminAddress();
    expect(admin_after).toEqual(deployer.address);
}

get fun `test-minter-admin-can-change-content`() {
    val (minter, deployer, _) = setup_test();

    // Get initial content
    val initial_content = minter.getContent();

    // Create new content
    val new_content = beginCell().storeUint(1, 1).storeSlice("https://totally_new_jetton.org/content.json").endCell();

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

get fun `test-not-a-minter-admin-can-not-change-content`() {
    val (minter, _, not_deployer) = setup_test();

    // Get initial content
    val initial_content = minter.getContent();

    // Try to change content as non-admin
    val new_content = beginCell().storeUint(1, 1).endCell();
    val change_result = minter.sendChangeContent(not_deployer.address, new_content);

    expect(change_result).toHaveFailedTx({
        from: not_deployer.address,
        to: minter.address,
        exit_code: ERR_NOT_FROM_ADMIN
    });

    // Check content didn't change
    expect(minter.getContent()).toEqual(initial_content);
}

get fun `test-wallet-owner-should-be-able-to-send-jettons`() {
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

get fun `test-not-wallet-owner-should-not-be-able-to-send-jettons`() {
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
        exit_code: ERR_NOT_FROM_OWNER
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(not_deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance_2);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test-impossible-to-send-too-much-jettons`() {
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
        exit_code: ERR_NOT_ENOUGH_BALANCE
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(not_deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance_2);
}

get fun `test-correctly-sends-forward-payload`() {
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

get fun `test-no-forward-ton-amount-no-forward`() {
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

get fun `test-check-revert-on-not-enough-tons-for-forward`() {
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
        exit_code: ERR_NOT_ENOUGH_TON
    });

    expect(send_result).toHaveBouncedTx({
        from: deployer_jetton_wallet.address,
        to: deployer.address,
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
}

get fun `test-wallet-does-not-accept-internal-transfer-not-from-wallet`() {
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
        exit_code: ERR_INVALID_WALLET
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
}

get fun `test-wallet-owner-should-be-able-to-burn-jettons`() {
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

get fun `test-not-wallet-owner-should-not-be-able-to-burn-jettons`() {
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
        exit_code: ERR_NOT_FROM_OWNER
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test-wallet-owner-can-not-burn-more-jettons-than-it-has`() {
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
        exit_code: ERR_NOT_ENOUGH_BALANCE
    });

    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(initial_jetton_balance);
    expect(minter.getTotalSupply()).toEqual(initial_total_supply);
}

get fun `test-minter-should-only-accept-burn-messages-from-jetton-wallets`() {
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
            burnInitiator: createAddressNone(), // wrong address
            sendExcessesTo: deployer.address,
        }
    });

    val send_result_wrong = net.send(deployer_jetton_wallet.address, burn_notification_wrong, SEND_MODE_PAY_FEES_SEPARATELY);

    expect(send_result_wrong).toHaveFailedTx({
        from: deployer_jetton_wallet.address,
        to: minter.address,
        exit_code: ERR_UNAUTHORIZED_BURN
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

get fun `test-report-correct-discovery-address`() {
    val (minter, deployer, not_deployer) = setup_test();

    // Test discovery with includeOwnerAddress = true for deployer
    val discovery_result_deployer = minter.sendDiscovery(deployer.address, deployer.address, true);
    val _ = userWallet(minter, deployer.address);

    expect(discovery_result_deployer).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address
    });

    // Test discovery with includeOwnerAddress = true for notDeployer
    val discovery_result_not_deployer = minter.sendDiscovery(deployer.address, not_deployer.address, true);
    val _ = userWallet(minter, not_deployer.address);
    // TODO: way to check body

    expect(discovery_result_not_deployer).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address
    });

    // Test discovery with includeOwnerAddress = false
    val discovery_result_no_include = minter.sendDiscovery(deployer.address, not_deployer.address, false);

    expect(discovery_result_no_include).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address
    });
}

@custom("todo")
get fun `test-wallet-should-restore-balance-on-internal-transfer-bounce`() {
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
    val tx_amount = 150; // random amount between 100-200

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

    val transfer_result = net.send(deployer.address, transfer_msg, SEND_MODE_PAY_FEES_SEPARATELY);
    expect(transfer_result).toHaveSuccessfulTx({
        from: deployer.address,
        to: deployer_jetton_wallet.address
    });

    // Check balance decreased
    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(balance_before - tx_amount);

    // Send bounced message back
    val bounced_msg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: deployer_jetton_wallet.address,
        body: beginCell().storeUint(0xFFFFFFFF, 32).storeSlice(beginCell().storeUint(0x178d4519, 32).storeUint(0, 64).storeCoins(tx_amount).storeAddress(deployer.address).storeAddress(deployer.address).storeCoins(ton("0.05")).storeUint(0, 1).endCell().beginParse()).endCell(),
    });

    val bounce_result = net.send(not_deployer_jetton_wallet.address, bounced_msg, SEND_MODE_PAY_FEES_SEPARATELY);
    expect(bounce_result).toHaveSuccessfulTx({
        from: not_deployer_jetton_wallet.address,
        to: deployer_jetton_wallet.address,
        exit_code: 0xFFFF,
    });

    // Balance should be restored
    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(balance_before);
}

@custom("todo")
get fun `test-wallet-should-restore-balance-on-burn-notification-bounce`() {
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
    val burn_amount = 150; // random amount between 100-200

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

    val burn_result = net.send(deployer.address, burn_msg, SEND_MODE_PAY_FEES_SEPARATELY);
    expect(burn_result).toHaveSuccessfulTx({
        from: deployer.address,
        to: deployer_jetton_wallet.address
    });

    // Check balance decreased
    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(balance_before - burn_amount);

    // Send bounced message back from minter
    val bounced_msg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: deployer_jetton_wallet.address,
        body: beginCell().storeUint(0xFFFFFFFF, 32).storeSlice(beginCell().storeUint(0x7bdd97de, 32).storeUint(0, 64).storeCoins(burn_amount).storeAddress(deployer.address).storeAddress(deployer.address).endCell().beginParse()).endCell(),
    });

    val bounce_result = net.send(minter.address, bounced_msg, SEND_MODE_PAY_FEES_SEPARATELY);
    expect(bounce_result).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer_jetton_wallet.address,
        exit_code: 0xFFFF,
    });

    // Balance should be restored
    expect(deployer_jetton_wallet.getJettonBalance()).toEqual(balance_before);
}

@custom("todo")
get fun `test-correctly-handles-not-valid-address-in-discovery`() {
    val (minter, deployer, _) = setup_test();

    // Create a bad address (workchain -1)
    val bad_address = beginCell().storeInt(-1, 8).storeUint(0, 256).endCell().beginParse().loadAddress();

    // Test discovery with bad address and includeOwnerAddress = false
    val discovery_result_bad_no_include = minter.sendDiscovery(deployer.address, bad_address, false);

    expect(discovery_result_bad_no_include).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address
    });

    // Test discovery with bad address and includeOwnerAddress = true
    val discovery_result_bad_include = minter.sendDiscovery(deployer.address, bad_address, true);

    expect(discovery_result_bad_include).toHaveSuccessfulTx({
        from: minter.address,
        to: deployer.address
    });
}

get fun `test-can-not-send-to-masterchain`() {
    val (minter, deployer, _) = setup_test();

    val deployer_jetton_wallet = userWallet(minter, deployer.address);

    val mint_result = minter.sendMint(
        deployer.address,
        deployer.address,
        ton("2"),
        ton("0.05"),
        ton("1")
    );
    expect(mint_result).toHaveSuccessfulTx({ from: minter.address, to: userWallet(minter, deployer.address).address });

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
        exit_code: ERR_WRONG_WORKCHAIN
    });
}

fun JettonWalletDataReply.fromTuple(data: tuple): JettonWalletDataReply asm "4 UNTUPLE"
fun JettonDataReply.fromTuple(data: tuple): JettonDataReply asm "5 UNTUPLE"

// TODO: single method
// fun T.fromTuple(self): T {
//     return untuple_any(sizeof(self)) as T
// }
//
// fun untuple_any<T>(count: int): T asm "UNTUPLEVAR"
