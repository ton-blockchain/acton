import type {ParsedValue, ParsedValueObjectEntry} from "@acton/ui"
import type {Address, Cell} from "@ton/core"

import {loadDNSRecord, type DNSRecord} from "../block.tlb.generated"

import type {DomainParser, DomainParserMatch} from "./types"

const scalar = (value: string, typeName?: string): ParsedValue => ({
  kind: "scalar",
  value,
  ...(typeName ? {typeName} : {}),
})

const address = (value: Address): ParsedValue => ({
  kind: "address",
  value: value.toRawString(),
})

const entry = (key: string, value: ParsedValue): ParsedValueObjectEntry => ({key, value})

function dnsRecordPresentation(record: DNSRecord): {
  readonly label: string
  readonly entries: readonly ParsedValueObjectEntry[]
} {
  switch (record.kind) {
    case "DNSRecord_dns_text":
      return {
        label: "Text",
        entries: [entry("Text chunks", scalar(String(record._.chunks), "uint8"))],
      }
    case "DNSRecord_dns_next_resolver":
      return {
        label: "Next resolver",
        entries: [entry("Resolver", address(record.resolver))],
      }
    case "DNSRecord_dns_adnl_address":
      return {
        label: "ADNL address",
        entries: [
          entry("ADNL address", scalar(record.adnl_addr.toString("hex"), "bits256")),
          entry("Flags", scalar(String(record.flags), "uint8")),
          entry("Protocols", scalar(record.proto_list ? "Present" : "None")),
        ],
      }
    case "DNSRecord_dns_smc_address":
      return {
        label: "Smart contract address",
        entries: [
          entry("Address", address(record.smc_addr)),
          entry("Flags", scalar(String(record.flags), "uint8")),
          entry("Capabilities", scalar(record.cap_list ? "Present" : "None")),
        ],
      }
    case "DNSRecord_dns_storage_address":
      return {
        label: "Storage address",
        entries: [entry("Bag ID", scalar(record.bag_id.toString("hex"), "bits256"))],
      }
    default: {
      const unsupportedRecord: never = record
      throw new Error(`Unsupported TON DNS record: ${String(unsupportedRecord)}`)
    }
  }
}

function parseDnsRecord(cell: Cell): DomainParserMatch | undefined {
  const wrappedInSlice = cell.bits.length === 0 && cell.refs.length === 1
  const recordCell = wrappedInSlice ? cell.refs[0] : cell
  const slice = recordCell.beginParse()

  let record: DNSRecord
  try {
    record = loadDNSRecord(slice)
  } catch {
    return undefined
  }

  // Constructor tags alone are not sufficient for automatic detection. Requiring
  // complete consumption prevents an embedded prefix from becoming a false match.
  if (slice.remainingBits !== 0 || slice.remainingRefs !== 0) {
    return undefined
  }

  const presentation = dnsRecordPresentation(record)
  return {
    data: record,
    parsedValue: {
      kind: "object",
      typeName: "TON DNS record",
      entries: [entry("Record type", scalar(presentation.label)), ...presentation.entries],
    },
    label: `TON DNS · ${presentation.label}`,
    details: {
      type: presentation.label,
      layout: wrappedInSlice ? "Referenced value" : "Direct value",
    },
    confidence: {
      score: 0.99,
      reasons: [
        "The DNS constructor tag matched",
        "The DNS record consumed the complete value",
        ...(wrappedInSlice ? ["The empty slice root contained exactly one value reference"] : []),
      ],
    },
  }
}

/** TON DNS record layouts with their semantic field presentation */
export const dnsDomainParser: DomainParser = {
  id: "ton-dns-record",
  parse: parseDnsRecord,
}
