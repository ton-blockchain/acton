use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const BALANCE_COUNTER_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun current_balance(): coins {
    return contract.getOriginalBalance();
}
";

const BALANCE_COUNTER_TEST: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/io"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromEmpty(): Counter {
    val init = ContractState {
        code: build("Counter", "contracts/Counter.tolk"),
        data: beginCell().endCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init };
}

fun setupTest(): (Counter, Treasury, SendResultList) {
    val contract = Counter.fromEmpty();
    val deployer = testing.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: { stateInit: contract.init },
    });
    val deployResult = net.send(deployer.address, msg);
    expect(deployResult).toHaveSuccessfulDeploy({ to: contract.address });
    return (contract, deployer, deployResult);
}

fun Counter.currentBalance(self): coins {
    return net.runGetMethod(self.address, "current_balance");
}

get fun `test balance getter`() {
    val (contract, deployer, _) = setupTest();
    val actualBalance = testing.getAccountBalance(contract.address);
    println("Actual balance: {}", actualBalance);

    expect(contract.currentBalance()).toEqual(actualBalance);
}
"#;

#[test]
fn get_method_current_balance_matches_account_balance() {
    ProjectBuilder::new("get-method-current-balance-matches-account-balance")
        .contract("Counter", BALANCE_COUNTER_CONTRACT)
        .test_file("balance_getter", BALANCE_COUNTER_TEST)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/get_method_current_balance_matches_account_balance/get_method_current_balance_matches_account_balance.stdout.txt",
        );
}
