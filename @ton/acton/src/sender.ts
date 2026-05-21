import {
  Address,
  Cell,
  packExtraCurrencyDict,
  type Message,
  type Sender,
  type SenderArguments,
} from "@ton/core"

import type {Localnet} from "./localnet.js"

export class LocalnetSender implements Sender {
  constructor(
    private readonly localnet: Pick<Localnet, "sendMessage">,
    readonly address: Address,
  ) {}

  async send(args: SenderArguments): Promise<void> {
    const message: Message = {
      body: args.body ?? Cell.EMPTY,
      init: args.init ?? undefined,
      info: {
        bounce: args.bounce ?? true,
        bounced: false,
        createdAt: 0,
        createdLt: 0n,
        dest: args.to,
        forwardFee: 0n,
        ihrDisabled: true,
        ihrFee: 0n,
        src: this.address,
        type: "internal",
        value: {
          coins: args.value,
          other: args.extracurrency ? packExtraCurrencyDict(args.extracurrency) : undefined,
        },
      },
    }

    await this.localnet.sendMessage(message)
  }
}
