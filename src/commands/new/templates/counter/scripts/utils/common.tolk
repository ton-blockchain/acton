import "@acton/env"
import "@contracts/types"
import "@wrappers/Counter.gen"

fun counterDeployValue(): coins {
    return env<coins>("COUNTER_DEPLOY_VALUE_NANOS") ?? grams("0.005");
}

fun explicitCounterAddress(): address? {
    return env("COUNTER_ADDRESS");
}

fun targetCounter(owner: address): Counter {
    val counterAddress = explicitCounterAddress();
    if (counterAddress != null) {
        return Counter.fromAddress(counterAddress);
    }
    return Counter.fromStorage({
        id: 0,
        owner,
        counter: 0,
    });
}
