use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const RANDOM_PROBE_CONTRACT: &str = r"
struct Storage {
    lastSeed: uint256
}

fun onInternalMessage(_: InMessage) {
    contract.setData(Storage {
        lastSeed: random.getSeed(),
    }.toCell());
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun lastSeed(): uint256 {
    return Storage.fromCell(contract.getData()).lastSeed;
}

get fun observedSeed(): uint256 {
    return random.getSeed();
}
";

const RANDOM_TICK_TOCK_CONTRACT: &str = r"
struct TickTockStorage {
    lastSeed: uint256
}

fun onRunTickTock(_: bool) {
    contract.setData(TickTockStorage {
        lastSeed: random.getSeed(),
    }.toCell());
}

fun onInternalMessage(_: InMessage) {}

fun onBouncedMessage(_: InMessageBounced) {}

get fun lastSeed(): uint256 {
    return TickTockStorage.fromCell(contract.getData()).lastSeed;
}
";

const TEST_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/types/big_array"
import "../../lib/types/transaction"

const RANDOM_SEED_BEFORE = 0x1234
const RANDOM_SEED_AFTER = 0x5678

struct Storage {
    lastSeed: uint256
}

struct TickTockStorage {
    lastSeed: uint256
}

fun deployProbe(): (Treasury, address) {
    val code = build("random_probe");
    val stateInit = ContractState {
        code,
        data: Storage { lastSeed: 0 }.toCell(),
    };
    val probe = AutoDeployAddress {
        stateInit,
    }.calculateAddress();
    val sender = testing.treasury("random-seed-sender");

    val deployTxs = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit,
        },
    }));
    expect(deployTxs).toHaveSuccessfulDeploy({
        from: sender.address,
        to: probe,
    });

    return (sender, probe);
}

fun deployTickTockProbe(): address {
    val code = build("random_tick_tock");
    val stateInit = ContractState {
        code,
        data: TickTockStorage { lastSeed: 0 }.toCell(),
    };
    val probe = AutoDeployAddress {
        stateInit,
    }.calculateAddress();
    val sender = testing.treasury("random-tick-tock-sender");

    val deployTxs = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit,
        },
    }));
    expect(deployTxs).toHaveSuccessfulDeploy({
        from: sender.address,
        to: probe,
    });

    return probe;
}

fun pingProbe(sender: Treasury, probe: address, opcode: int): void {
    val txs = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: probe,
        body: beginCell().storeUint(opcode, 24).endCell(),
    }));
    expect(txs).toHaveSuccessfulTx({
        from: sender.address,
        to: probe,
    });
}
"#;

#[test]
fn testing_set_random_seed_controls_current_and_emulated_vm_seed() {
    let source = format!(
        r#"
        {TEST_IMPORTS}

        get fun `test testing set random seed controls current and emulated vm seed`() {{
            val generated = testing.setRandomSeed();
            expect(random.getSeed()).toEqual(generated);

            testing.setRandomSeed(RANDOM_SEED_BEFORE);
            expect(random.getSeed()).toEqual(RANDOM_SEED_BEFORE);

            val (sender, probe) = deployProbe();
            val tickTockProbe = deployTickTockProbe();

            val generatedForEmulation = testing.setRandomSeed();
            expect(random.getSeed()).toEqual(generatedForEmulation);
            val generatedGetterSeed = net.runGetMethod<uint256>(probe, "observedSeed");
            testing.setRandomSeed(generatedForEmulation);
            expect(net.runGetMethod<uint256>(probe, "observedSeed")).toEqual(generatedGetterSeed);

            testing.setRandomSeed(RANDOM_SEED_BEFORE);
            val getterSeedBefore = net.runGetMethod<uint256>(probe, "observedSeed");
            testing.setRandomSeed(RANDOM_SEED_BEFORE);
            expect(net.runGetMethod<uint256>(probe, "observedSeed")).toEqual(getterSeedBefore);
            expect(testing.saveSnapshot("random-seed-before.json")).toBeTrue();

            testing.setRandomSeed(RANDOM_SEED_BEFORE);
            pingProbe(sender, probe, 0x5EED01);
            val txSeedBefore = net.runGetMethod<uint256>(probe, "lastSeed");
            testing.setRandomSeed(RANDOM_SEED_BEFORE);
            pingProbe(sender, probe, 0x5EED02);
            expect(net.runGetMethod<uint256>(probe, "lastSeed")).toEqual(txSeedBefore);

            testing.setRandomSeed(RANDOM_SEED_BEFORE);
            expect(testing.runTickTock(tickTockProbe, false).size()).toBeGreater(0);
            val tickTockSeedBefore = net.runGetMethod<uint256>(tickTockProbe, "lastSeed");
            testing.setRandomSeed(RANDOM_SEED_BEFORE);
            expect(testing.runTickTock(tickTockProbe, false).size()).toBeGreater(0);
            expect(net.runGetMethod<uint256>(tickTockProbe, "lastSeed")).toEqual(tickTockSeedBefore);
            println("seed before: {{:X}}", RANDOM_SEED_BEFORE);

            testing.setRandomSeed(RANDOM_SEED_AFTER);
            val getterSeedAfter = net.runGetMethod<uint256>(probe, "observedSeed");
            expect(getterSeedAfter).toNotEqual(getterSeedBefore);

            pingProbe(sender, probe, 0x5EED03);
            expect(net.runGetMethod<uint256>(probe, "lastSeed")).toNotEqual(txSeedBefore);

            expect(testing.runTickTock(tickTockProbe, false).size()).toBeGreater(0);
            expect(net.runGetMethod<uint256>(tickTockProbe, "lastSeed")).toNotEqual(tickTockSeedBefore);
            println("seed after: {{:X}}", RANDOM_SEED_AFTER);

            expect(testing.loadSnapshot("random-seed-before.json")).toBeTrue();
            expect(net.runGetMethod<uint256>(probe, "observedSeed")).toEqual(getterSeedBefore);
        }}
        "#
    );

    ProjectBuilder::new("testing-set-random-seed-controls-vm-seed")
        .contract("random_probe", RANDOM_PROBE_CONTRACT)
        .contract("random_tick_tock", RANDOM_TICK_TOCK_CONTRACT)
        .test_file("testing_random_seed", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/testing_set_random_seed_controls_emulated_vm_seed/testing_set_random_seed_controls_current_and_emulated_vm_seed.stdout.txt",
        );
}
