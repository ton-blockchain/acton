import type {LanguageSupport} from "../types"

export const TASM_LANGUAGE_ID = "tasm"
export const TASM_SPEC_URL = "/tvm-specification.json"

export const tasmLanguageSupport = {
  id: TASM_LANGUAGE_ID,
  label: "TASM",
  fileExtension: "tasm",
  defaultSource: `SETCP 0
DICTPUSHCONST 19 [
    0 => {
        INMSG_BOUNCED
        PUSHCONT_SHORT {
            DROP
        }
        IFJMP
        DUP
        SDBEGINSQ x{7E8764EF}
        PUSHCONT {
            NIP
            PUSHCTR c4
            CTOS
            SWAP
            LDU 64
            NIP
            PLDU 32
            SWAP
            LDU 32
            PLDU 32
            ROT
            ADD
            OVER
            NEWC
            STU 32
            POP s2
            PUXC s0 s1
            STU 32
            NIP
            ENDC
            POPCTR c4
        }
        IFJMP
        SDBEGINSQ x{3A752F06}
        NIP
        PUSHCONT {
            DROP
            PUSHCTR c4
            CTOS
            PLDU 32
            DUP
            NEWC
            STU 32
            NIP
            STSLICECONST x{00000000}
            ENDC
            POPCTR c4
        }
        IFJMP
        PUSHPOW2DEC 16
        SWAP
        SEMPTY
        THROWANYIFNOT
    }
    71937 => {
        PUSHCTR c4
        CTOS
        PLDU 32
    }
    117456 => {
        PUSHCTR c4
        CTOS
        LDU 32
        NIP
        PLDU 32
    }
]
DICTIGETJMPZ
THROWARG 11`,
  extensionPoint: {
    id: TASM_LANGUAGE_ID,
    aliases: ["TASM", "tasm"],
    extensions: [".tasm"],
  },
  monarchLanguage: {
    tokenizer: {
      root: [
        [/;.*$/, "comment"],
        [/(?:x\{[0-9a-fA-F]*_?\}|b\{[01]*\}|boc\{[0-9a-fA-F]*\})/, "number.hex"],
        [/"(?:[^"\\]|\\.)*"/, "string"],
        [/[{}[\](),]/, "delimiter"],
        [/-?\d+/, "number"],
        [/[cCsS]\d+/, "variable.predefined"],
        [/[A-Z][A-Z0-9_]+/, "keyword"],
        [/[a-z_][A-Za-z0-9_]*/, "identifier"],
      ],
    },
  },
} as const satisfies LanguageSupport<typeof TASM_LANGUAGE_ID>
