import "@acton/env"
import "@acton/prompts"
import "@acton/emulation/scripts"

fun promptW5Wallet(envName: string, message: string): ActonWallet {
    return scripts.wallet(env<string>(envName) ?? promptWallet(message));
}

fun promptW5Address(envName: string, message: string): address {
    return env<address>(envName) ?? promptAddress(message);
}
