pub(super) fn annotation(name: &str) -> Option<&'static str> {
    let description = match name {
        "inline" => {
            "Function with this annotation will be automatically inlined during compilation"
        }
        "inline_ref" => {
            "Function with this annotation will be automatically inlined by reference during \
             compilation"
        }
        "noinline" => {
            "Function with this annotation will not be inlined even if compiler can inline it"
        }
        "pure" => {
            "Function with this annotation has no side effects and can be optimized away by the \
             compiler"
        }
        "deprecated" => {
            "Symbol with this annotation is deprecated and should not be used in new code. First \
             string argument is a reason for deprecation as a string literal."
        }
        "overflow1023_policy" => {
            "Defines the policy for handling potential builder overflow. Right now, only \
             `\"suppress\"` value is supported. See \
             <https://docs.ton.org/v3/documentation/smart-contracts/tolk/tolk-vs-func/pack-to-from-cells#what-if-data-exceeds-1023-bits> \
             for more details"
        }
        "on_bounced_policy" => {
            "Defines the policy for handling bounced messages. Right now, only `\"manual\"` value \
             is supported."
        }
        "method_id" => {
            "Specifies the method ID (as a number literal) for the function in smart contract \
             interface. See <https://docs.ton.org/v3/guidelines/smart-contracts/get-methods> for \
             more details"
        }
        "abi" => "Describes ABI metadata for a declaration.",
        "abi.minimalMsgValue" => {
            "Defines the minimal message value for a message struct in ABI metadata."
        }
        "abi.preferredSendMode" => {
            "Defines the preferred send mode for a message struct in ABI metadata."
        }
        "abi.clientType" => {
            "Overrides the client-facing ABI type for a struct field. This is useful when generated \
             wrappers should expose a different representation than the serialized Tolk field type."
        }
        "test" => {
            "Describes additional metadata for a test function, such as skipping, TODO state, \
             expected exit code, gas limit, or fuzzing configuration."
        }
        "test.skip" => "Marks the test as skipped.",
        "test.todo" => "Marks the test as TODO. Use `@test.todo(\"...\")` to attach a description.",
        "test.fail_with" => "Declares the expected exit code for the test.",
        "test.gas_limit" => "Overrides the per-test gas limit.",
        "test.fuzz" => {
            "Enables fuzzing for parameterized tests. Supports `@test.fuzz`, `@test.fuzz(64)`, and \
             `@test.fuzz({ ... })`."
        }
        _ => return name.split_once('.').and_then(|(root, _)| annotation(root)),
    };

    Some(description)
}

pub(super) fn contract_field(name: &str) -> Option<&'static str> {
    Some(match name {
        "contractName" => "Name of the contract.",
        "author" => "Author of the contract.",
        "version" => "Version of the contract.",
        "description" => "Description of the contract.",
        "incomingMessages" => {
            "Defines the type of allowed incoming internal messages. Usually a union type of all \
             supported message structs."
        }
        "incomingExternal" => "Defines the type of allowed incoming external messages.",
        "storage" => {
            "Defines the persistent storage structure for the contract. This field usually points \
             to a struct type."
        }
        "storageAtDeployment" => "Defines the storage structure at the moment of deployment.",
        "forceAbiExport" => "List of types to additionally export to ABI.",
        _ => return None,
    })
}

pub(super) fn tlb_type(name: &str) -> Option<String> {
    let info = known_tlb_type(name).or_else(|| arbitrary_int_type(name))?;
    let description = info
        .description
        .map(|description| format!("\n\n{description}"))
        .unwrap_or_default();

    Some(format!(
        "- **Range**: {}\n- **Size**: {}\n- **TL-B**: {}{}",
        info.range, info.size, name, description
    ))
}

struct TlbTypeInfo {
    range: String,
    size: String,
    description: Option<&'static str>,
}

fn known_tlb_type(name: &str) -> Option<TlbTypeInfo> {
    let (range, size) = match name {
        "uint8" => ("0 to 255 (2^8 - 1)", "8 bits = 1 byte"),
        "uint16" => ("0 to 65,535 (2^16 - 1)", "16 bits = 2 bytes"),
        "uint32" => ("0 to 4,294,967,295 (2^32 - 1)", "32 bits = 4 bytes"),
        "uint64" => ("0 to 2^64 - 1", "64 bits = 8 bytes"),
        "uint128" => ("0 to 2^128 - 1", "128 bits = 16 bytes"),
        "uint256" => ("0 to 2^256 - 1", "256 bits = 32 bytes"),
        "int8" => ("-128 to 127 (-2^7 to 2^7 - 1)", "8 bits = 1 byte"),
        "int16" => ("-32,768 to 32,767 (-2^15 to 2^15 - 1)", "16 bits = 2 bytes"),
        "int32" => ("-2^31 to 2^31 - 1", "32 bits = 4 bytes"),
        "int64" => ("-2^63 to 2^63 - 1", "64 bits = 8 bytes"),
        "int128" => ("-2^127 to 2^127 - 1", "128 bits = 16 bytes"),
        "int256" => ("-2^255 to 2^255 - 1", "256 bits = 32 bytes"),
        "int257" => ("-2^256 to 2^256 - 1", "257 bits = 32 bytes + 1 bit"),
        "varuint16" => ("0 to 2^120 - 1", "4 to 124 bits"),
        "varint16" => ("-2^119 to 2^119 - 1", "4 to 124 bits"),
        "varuint32" => ("0 to 2^248 - 1", "5 to 253 bits"),
        "varint32" => ("-2^247 to 2^247 - 1", "5 to 253 bits"),
        _ => return None,
    };

    Some(TlbTypeInfo {
        range: range.to_owned(),
        size: size.to_owned(),
        description: None,
    })
}

fn arbitrary_int_type(name: &str) -> Option<TlbTypeInfo> {
    let (prefix, width) = if let Some(width) = name.strip_prefix("uint") {
        ("uint", width)
    } else if let Some(width) = name.strip_prefix("int") {
        ("int", width)
    } else {
        return None;
    };
    let width = width.parse::<u16>().ok()?;

    if (prefix == "uint" && !(1..=256).contains(&width))
        || (prefix == "int" && !(1..=257).contains(&width))
    {
        return None;
    }

    if prefix == "uint" {
        Some(TlbTypeInfo {
            range: format!("0 to 2^{width} - 1"),
            size: format!("{width} bits"),
            description: Some("Arbitrary bit-width unsigned integer type"),
        })
    } else {
        Some(TlbTypeInfo {
            range: format!("-2^{} to 2^{} - 1", width - 1, width - 1),
            size: format!("{width} bits"),
            description: Some("Arbitrary bit-width signed integer type"),
        })
    }
}
