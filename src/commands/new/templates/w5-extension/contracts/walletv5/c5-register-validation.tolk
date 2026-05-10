// The wallet contract is low-level.
// For instance, it operates with the TVM register c5 (out actions) and validates actions before pushing there.
// This file is dedicated to parsing and validating out actions.

import "errors"
import "messages"

/// put a raw list of OutActions to C5 register
// acton-disable-next-line asm-function-missing-safety-comment
fun setTvmRegisterC5(actionList: cell): void
    asm "c5 POP"

/// transform an ordinary or exotic cell into a slice, as if it were an ordinary cell
// acton-disable-next-line asm-function-missing-safety-comment
fun OutActionsCell.beginParseAllowExotic(self): slice
    asm "XCTOS" "DROP"

/// verify a cell containing OutActionList that it contains only "send message" actions
fun OutActionsCell.verifyC5Actions(self, isExternal: bool): self {
    var count = 0;

    var cs = self.beginParseAllowExotic();
    do {
        // these checks could be cheaper in terms of gas if we just try to parse cs;
        // but we want to throw exactly `Errors.InvalidC5` (not `9` "cell underflow")
        val (nBits, nRefs) = cs.remainingBitsAndRefsCount();
        assert (nRefs == 2) throw Errors.InvalidC5;
        assert (nBits == 32 + 8) throw Errors.InvalidC5;

        val outAction = lazy OutActionWithSendMessageOnly.fromSlice(
            cs,
            { throwIfOpcodeDoesNotMatch: Errors.InvalidC5 },
        );

        if (isExternal) {
            assert (
                (outAction.sendMode & SEND_MODE_IGNORE_ERRORS) != 0
            ) throw Errors.ExternalSendMessageMustHaveIgnoreErrorsSendMode;
        }

        cs = outAction.prev.beginParseAllowExotic();
        count += 1;
    } while (!cs.isEmpty());

    assert (count <= 255) throw Errors.InvalidC5;
    return self;
}
