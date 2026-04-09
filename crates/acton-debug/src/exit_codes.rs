use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct ExitCodeInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub phase: &'static str,
}

pub static EXIT_CODE_DESCRIPTIONS: LazyLock<HashMap<i32, ExitCodeInfo>> = LazyLock::new(|| {
    let mut map = HashMap::new();

    map.insert(
        0,
        ExitCodeInfo {
            name: "Success",
            description: "Standard successful execution exit code",
            phase: "Compute and action phases",
        },
    );

    map.insert(
        1,
        ExitCodeInfo {
            name: "Alt Success",
            description: "Alternative successful execution exit code. Reserved, but does not occur",
            phase: "Compute phase",
        },
    );

    map.insert(
        2,
        ExitCodeInfo {
            name: "Stack Underflow",
            description: "Stack underflow",
            phase: "Compute phase",
        },
    );

    map.insert(
        3,
        ExitCodeInfo {
            name: "Stack Overflow",
            description: "Stack overflow",
            phase: "Compute phase",
        },
    );

    map.insert(
        4,
        ExitCodeInfo {
            name: "Integer Overflow",
            description: "Integer overflow",
            phase: "Compute phase",
        },
    );

    map.insert(
        5,
        ExitCodeInfo {
            name: "Range Check Error",
            description: "Range check error — an integer is out of its expected range",
            phase: "Compute phase",
        },
    );

    map.insert(
        6,
        ExitCodeInfo {
            name: "Invalid Opcode",
            description: "Invalid TVM opcode",
            phase: "Compute phase",
        },
    );

    map.insert(
        7,
        ExitCodeInfo {
            name: "Type Check Error",
            description: "Type check error",
            phase: "Compute phase",
        },
    );

    map.insert(
        8,
        ExitCodeInfo {
            name: "Cell Overflow",
            description: "Cell overflow",
            phase: "Compute phase",
        },
    );

    map.insert(
        9,
        ExitCodeInfo {
            name: "Cell Underflow",
            description: "Cell underflow",
            phase: "Compute phase",
        },
    );

    map.insert(
        10,
        ExitCodeInfo {
            name: "Dictionary Error",
            description: "Dictionary error",
            phase: "Compute phase",
        },
    );

    map.insert(
        11,
        ExitCodeInfo {
            name: "Unknown Error",
            description: "Unknown error, may be thrown by user programs",
            phase: "Compute phase",
        },
    );

    map.insert(
        12,
        ExitCodeInfo {
            name: "Fatal Error",
            description: "Fatal error. Thrown by TVM in situations deemed impossible",
            phase: "Compute phase",
        },
    );

    map.insert(
        13,
        ExitCodeInfo {
            name: "Out of Gas",
            description: "Out of gas error",
            phase: "Compute phase",
        },
    );

    map.insert(
        -14,
        ExitCodeInfo {
            name: "Out of Gas (Negative)",
            description: "Out of gas error. Negative, so that it cannot be faked",
            phase: "Compute phase",
        },
    );

    map.insert(
        14,
        ExitCodeInfo {
            name: "VM Virtualization",
            description: "VM virtualization error. Reserved, but never thrown",
            phase: "Compute phase",
        },
    );

    map.insert(
        32,
        ExitCodeInfo {
            name: "Invalid Action List",
            description: "Action list is invalid",
            phase: "Action phase",
        },
    );

    map.insert(
        33,
        ExitCodeInfo {
            name: "Action List Too Long",
            description: "Action list is too long",
            phase: "Action phase",
        },
    );

    map.insert(
        34,
        ExitCodeInfo {
            name: "Invalid Action",
            description: "Action is invalid or not supported",
            phase: "Action phase",
        },
    );

    map.insert(
        35,
        ExitCodeInfo {
            name: "Invalid Source Address",
            description: "Invalid source address in outbound message",
            phase: "Action phase",
        },
    );

    map.insert(
        36,
        ExitCodeInfo {
            name: "Invalid Destination Address",
            description: "Invalid destination address in outbound message",
            phase: "Action phase",
        },
    );

    map.insert(
        37,
        ExitCodeInfo {
            name: "Not Enough Toncoin",
            description: "Not enough Toncoin",
            phase: "Action phase",
        },
    );

    map.insert(
        38,
        ExitCodeInfo {
            name: "Not Enough Extra Currencies",
            description: "Not enough extra currencies",
            phase: "Action phase",
        },
    );

    map.insert(
        39,
        ExitCodeInfo {
            name: "Message Too Large",
            description: "Outbound message does not fit into a cell after rewriting",
            phase: "Action phase",
        },
    );

    map.insert(
        40,
        ExitCodeInfo {
            name: "Cannot Process Message",
            description: "Cannot process a message — not enough funds, the message is too large, or its Merkle depth is too big",
            phase: "Action phase",
        },
    );

    map.insert(
        41,
        ExitCodeInfo {
            name: "Library Reference Null",
            description: "Library reference is null during library change action",
            phase: "Action phase",
        },
    );

    map.insert(
        42,
        ExitCodeInfo {
            name: "Library Change Error",
            description: "Library change action error",
            phase: "Action phase",
        },
    );

    map.insert(
        43,
        ExitCodeInfo {
            name: "Library Limits Exceeded",
            description: "Exceeded the maximum number of cells in the library or the maximum depth of the Merkle tree",
            phase: "Action phase",
        },
    );

    map.insert(
        50,
        ExitCodeInfo {
            name: "Account Size Exceeded",
            description: "Account state size exceeded limits",
            phase: "Action phase",
        },
    );

    map.insert(
        63,
        ExitCodeInfo {
            name: "Type prefix mismatch",
            description: "Unable to load data from cell because prefix does not match",
            phase: "Compute phase",
        },
    );

    map.insert(
        65535,
        ExitCodeInfo {
            name: "InvalidMessage",
            description: "Invalid message",
            phase: "Compute phase",
        },
    );

    map
});

#[must_use]
pub fn find(code: i32) -> Option<&'static ExitCodeInfo> {
    EXIT_CODE_DESCRIPTIONS.get(&code)
}
