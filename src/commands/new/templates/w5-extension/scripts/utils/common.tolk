import "@acton/env"
import "@acton/prompts"
import "@acton/emulation/scripts"

fun promptW5Wallet(envName: string, message: string): ActonWallet {
    val nameFromEnv = env<string>(envName);
    if (nameFromEnv != null) {
        return scripts.wallet(nameFromEnv);
    }
    return scripts.wallet(promptWallet(message));
}
