import "message"
import "../tlb/either"
import "../vm/vm"

struct OutList {
    action: OutAction
}

struct (0x0ec3c86d) OutActionSendMessage {
    next: Cell<OutList>
    mode: uint8,
    out_msg: cell
}

fun OutActionSendMessage.loadMessage<Msg>(self): MessageRelaxed<Msg> {
    return (self.out_msg as Cell<MessageRelaxed<Msg>>).load({
        assertEndAfterReading: false,
    });
}

fun OutActionSendMessage.loadBody<Msg>(self): Msg {
    return self.loadMessage<Msg>().loadBody()
}

fun OutActionSendMessage.loadGenericMessage(self): MessageRelaxedGeneric {
    return (self.out_msg as Cell<MessageRelaxedGeneric>).load({
        assertEndAfterReading: false,
    });
}

struct (0xad4de08e) OutActionSetCode {
    next: Cell<OutList>
    new_code: cell
}

type OutAction = OutActionSendMessage | OutActionSetCode

fun OutActionSendMessage.fromTuple(data: tuple): OutActionSendMessage asm "3 UNTUPLE"
fun OutActionSetCode.fromTuple(data: tuple): OutActionSetCode asm "2 UNTUPLE"
fun OutAction.fromTuple(data: tuple): OutAction {
    if (data.size() == 2) {
        return OutActionSetCode.fromTuple(data)
    } else {
        return OutActionSendMessage.fromTuple(data)
    }
}

fun OutAction.kind(self): slice {
    match (self) {
        OutActionSendMessage => {
            return "send-message"
        }
        OutActionSetCode => {
            return "set-code"
        }
    }
}

type OutActionList = tuple

fun OutActionList.at(self, i: int): OutAction {
    val raw_action = self.get<tuple>(i);
    return OutAction.fromTuple(raw_action);
}

fun OutActionList.getSendMessageAt(self, i: int): OutActionSendMessage? {
    val action = self.at(i);
    if (action is OutActionSendMessage) {
        return action
    }
    return null
}

fun OutActionList.getSendMessageBodyAt<Msg>(self, i: int): Msg? {
    val action = self.at(i);
    if (action is OutActionSendMessage) {
        val msg = (action.out_msg as Cell<MessageRelaxed<Msg>>).load({
            assertEndAfterReading: false,
        });

        if (msg.body is EitherLeft<Msg>) {
            return msg.body.value
        }
        if (msg.body is EitherRight<Cell<Msg>>) {
            return msg.body.value.load({
                assertEndAfterReading: false
            })
        }
        return null
    }
    return null
}

fun OutMessage.outActions(self, mode: int): OutActionList {
    // val prev_c5 = vm.getC5();
    self.send(mode);
    val list = vm.outActions();
    // vm.setC5(prev_c5);
    return list;
}

fun vm.outActions(): OutActionList {
    return vm.parseOutActions(vm.getC5())
}

fun vm.parseOutActions(cell: cell): OutActionList {
    var result = createEmptyTuple();
    val root = cell as Cell<OutList>;
    var next: Cell<OutList>? = root;

    while (next != null) {
        val loaded = next.load();
        match (loaded.action) {
            OutActionSendMessage => {
                result.push(loaded.action.__toTuple());
                if (loaded.action.next.beginParse().remainingBitsCount() == 0) {
                    next = null
                } else {
                    next = loaded.action.next;
                }
            }
            OutActionSetCode => {
                result.push(loaded.action.__toTuple());
                if (loaded.action.next.beginParse().remainingBitsCount() == 0) {
                    next = null
                } else {
                    next = loaded.action.next;
                }
            }
        }
    }

    return result;
}
