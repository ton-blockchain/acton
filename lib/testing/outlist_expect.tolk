/// Module for asserting properties of out action lists.
///
/// This module extends the `Expectation` class specifically for `OutActionList`.
/// `OutActionList` represents the actions (messages, code updates, etc.) produced by a contract execution.
///
/// Examples:
/// ```tolk
/// val actions = vm.outActions();
///
/// // Check if any actions were produced
/// expect(actions).toBeNonEmpty();
///
/// // Verify the first action is a message of type `TransferNotification`
/// expect(actions).toBeSendMessageAt<TransferNotification>(0);
///
/// // Accessing the body for further checks (using helper methods from `OutActionList` directly)
/// val msgBody = actions.getSendMessageBodyAt<TransferNotification>(0);
/// expect(msgBody).toBeNotNull();
/// expect(msgBody.amount).toEqual(ton("10"));
/// ```

import "assert"
import "expect"
import "../fmt"
import "../ffi/ffi"
import "../types/message"
import "../types/out_actions"

/// Expect the out action list to be non empty.
///
/// Example:
/// ```tolk
/// val out_actions = vm.outActions();
/// expect(out_actions).toBeNonEmpty();
/// ```
fun Expectation<OutActionList>.toBeNonEmpty(self) {
    expect(self.value.size(), self.location).toNotEqual(0);
}

/// Expect the out action list to be empty.
///
/// Example:
/// ```tolk
/// val out_actions = vm.outActions();
/// expect(out_actions).toBeEmpty();
/// ```
fun Expectation<OutActionList>.toBeEmpty(self) {
    expect(self.value.size(), self.location).toEqual(0);
}

/// Expect the out action at the given index to be a send message.
///
/// Example:
/// ```tolk
/// val out_actions = vm.outActions();
/// expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);
/// ```
///
/// Note that you should provide the message type as a generic parameter.
///
/// If the action is not a send message, the test will fail.
fun Expectation<OutActionList>.toBeSendMessageAt<Msg>(self, index: int) {
    val action = self.value.at(index);
    if (action is OutActionSendMessage) {
        try {
            action.loadMessage<Msg>();
        } catch (e) {
            if (e == 63) {
                // prefix mismatch
                val msg = action.loadGenericMessage();
                val opcode = msg.loadOpcode();
                if (opcode == null) {
                    expectImpl.failToFindOutMsg<Msg>(e, self.location);
                    return;
                }

                val opcodeTypeName = ffi.typeNameByOpcode(opcode);

                Assert.fail(
                    format4(
                        "Out action is a send message, but it cannot be load as a `{}` (opcode: 0x{:x}) since body has opcode 0x{:x}{}",
                        Msg.typeName(),
                        Msg.getDeclaredPackPrefix(),
                        opcode,
                        opcodeTypeName == null ? "" : format1(" ({})", opcodeTypeName),
                    ),
                    self.location
                );
                return;
            }
            expectImpl.failToFindOutMsg<Msg>(e, self.location);
        }
        return;
    }

    Assert.fail(
        format2("Out action at index {} is not a send message, but {}", index, action.kind()),
        self.location
    );
}

struct expectImpl

fun expectImpl.failToFindOutMsg<Msg>(exitCode: int, location: slice) {
    Assert.fail(
        format2(
            "Out action is a send message, but it cannot be load as a `{}` due to exit code {}",
            Msg.typeName(),
            exitCode
        ),
        location
    );
}
