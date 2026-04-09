/// Module for asserting properties of out action lists.
///
/// This module extends the `Expectation` class specifically for `OutActionList`.
/// `OutActionList` represents the actions (messages, code updates, etc.) produced by a contract execution.
/// Indexing follows `OutActionList` semantics: index `0` is the last produced action.
///
/// Examples:
/// ```tolk
/// val actions = vm.outActions();
///
/// // Check if any actions were produced
/// expect(actions).toBeNonEmpty();
///
/// // Verify the last produced action is a message of type `TransferNotification`
/// // (`OutActionList` is reversed, so this is index 0)
/// expect(actions).toBeSendMessageAt<TransferNotification>(0);
///
/// // Accessing the body for further checks (using helper methods from `OutActionList` directly)
/// val msgBody = actions.getSendMessageBodyAt<TransferNotification>(0);
/// expect(msgBody).toBeNotNull();
/// expect(msgBody.amount).toEqual(ton("10"));
/// ```

import "@stdlib/reflection"
import "../fmt"
import "../ffi/ffi"
import "../types/message"
import "../types/out_actions"
import "assert"
import "expect"

/// Expect the out action at the given index to be a send message that can be loaded as `MessageRelaxed<Msg>`.
///
/// Indexing is reverse-chronological (same as `OutActionList.at`): index `0` is the last produced action.
///
/// Example:
/// ```tolk
/// val out_actions = vm.outActions();
/// expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);
/// ```
///
/// Note that you should provide the message type as a generic parameter.
///
/// This matcher validates action kind and typed message loading only.
/// It does not call `loadBody()`, so it does not validate message body fields.
/// For body-level checks, load and validate the body separately.
///
/// Failure diagnostics:
/// - if the action is not a send message, assertion fails with index information;
/// - if typed load fails with opcode mismatch (exit code `63`), assertion includes expected type/prefix and actual opcode (plus known type name when available);
/// - for other decode errors, assertion includes the decoder exit code.
fun Expectation<OutActionList>.toBeSendMessageAt<Msg>(self, index: int): void {
    val action = self.value.at(index);
    if (action is OutActionSendMessage) {
        try {
            action.loadMessage<Msg>();
        } catch (e) {
            if (e == 63) {
                // prefix mismatch
                val msg = action.loadGenericMessage();
                val opcode = msg.loadOpcode(true);
                if (opcode == null) {
                    expectImpl.failToFindOutMsg<Msg>(e, self.location);
                    return;
                }

                val opcodeTypeName = ffi.typeNameByOpcode(opcode);

                Assert.fail(
                    format(
                        "Out action is a send message, but it cannot be load as a `{}` (opcode: 0x{:x}) since body has opcode 0x{:x}{}",
                        reflect.typeNameOf<Msg>(),
                        reflect.serializationPrefixOf<Msg>().0,
                        opcode,
                        opcodeTypeName == null ? "" : format(" ({})", opcodeTypeName),
                    ),
                    self.location,
                );
                return;
            }
            expectImpl.failToFindOutMsg<Msg>(e, self.location);
        }
        return;
    }

    Assert.fail(format("Out action at index {} is not a send message", index), self.location);
}

struct expectImpl

fun expectImpl.failToFindOutMsg<Msg>(exitCode: int, location: string): never {
    Assert.fail(
        format(
            "Out action is a send message, but it cannot be load as a `{}` due to exit code {}",
            reflect.typeNameOf<Msg>(),
            exitCode,
        ),
        location,
    );
}
