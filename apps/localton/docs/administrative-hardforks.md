# Change account state with administrative hardforks

Use **Admin actions** in Acton Studio to prepare account states for contract tests
on a full localnet. You can set a balance, replace contract code or data, freeze
an account, or delete it.

Each operation creates a **real hardfork** of your local blockchain. All managed
nodes accept a new block with the edited account state, then continue normal
block production. The change persists in node storage and appears in Explorer.

An administrative edit changes state directly. It does not execute the contract,
send messages, or create a transaction. The account transaction history therefore
has no transaction for this change.

## Before you apply an edit

- Start your full localnet and all its nodes. Every node must be available and
  synchronized before the operation starts
- Use a Localton image with support for administrative hardforks and account
  indexing after a hardfork. Studio checks image support before it pauses the network
- Allow enough disk space for recovery snapshots of every node. The operation
  copies node databases and can take several minutes
- For a restore point that you can use later, create a snapshot on **Snapshots**
- If your test requires a stable account state after the edit, stop external
  scripts that send messages

Existing environments keep the image that you selected at creation. A new default
image does not update them. If Studio reports an unsupported image, create an
environment with a compatible image.

## Apply an account change

- Open your full localnet in Studio, then open **Admin actions**
- Select an **Action** from the table below
- Enter the **Account address**, or select a wallet or known contract from the suggestions
- Enter the balance or BoC required for the action
- Select **Apply changes**

| Action | Result | Input |
| --- | --- | --- |
| Set balance | Sets the native balance to an exact amount | A nonnegative GRAM amount, with up to 9 decimal places |
| Replace code | Replaces code and preserves data and balance. An uninitialized account becomes active | The code cell |
| Replace data | Replaces data and preserves code and balance. The account must be active | The data cell |
| Freeze account | Replaces active state with its StateInit hash and preserves the balance | No additional value |
| Make account uninitialized | Removes code and data and preserves the balance | No additional value |
| Delete account | Removes the account, including its balance and state | No additional value |
| Replace complete ShardAccount | Replaces the balance, state and transaction reference | A complete ShardAccount with the target address |

For example, **Set balance** with `42` sets the account balance to **42 GRAM**.
It does not add 42 GRAM to the current balance.

The address field accepts raw and user-friendly addresses. The BoC field accepts
base64, base64url, hex, and links that contain an encoded BoC. **Load file** accepts
binary BoC files and text files. The input must contain one ordinary root cell.

**Replace complete ShardAccount** requires the serialized account record.
A code cell, data cell, or StateInit alone is not a complete ShardAccount.

## Progress and completion

Studio stops the activity generator and pauses the network. It saves recovery
snapshots, applies the same hardfork on every node, and checks the resulting state.
It then resumes block production and waits for Explorer indexing to reach the
changed block. The activity generator remains stopped until you start it again.

The progress notification shows the current step. **Changes applied** includes a
link to the masterchain block where Studio verified the change. Your form values
remain available after the operation.

You can leave the page or close the browser. The localnet service continues the
operation. The page shows progress again if you return before completion.

Studio and the Acton CLI block conflicting network, node and snapshot changes
until the operation finishes. Avoid manual Docker operations or direct Localton
commands during this time.

## Failed or interrupted operations

If an operation fails after the network pauses, Studio attempts to restore every
node from its recovery snapshot. It rebuilds the index before the environment
becomes available again. The error notification reports a failed restore separately.

If the connection fails before Studio receives a response, **Retry same operation**
sends the original request again. The service recognizes that request and does
not create a second hardfork for it.

After a localnet service restart, automatic recovery runs before normal startup.
An interrupted operation appears as failed. Inspect the account in Explorer
before you submit another edit.

Recovery snapshots are separate from snapshots on the **Snapshots** page. They
remain in the environment's `localton-snapshots` Docker volume under
`admin/<operation-id>/<service>/` and use disk space after completion.

If recovery fails, retain the environment data, recovery snapshots and
`admin-recovery.json`. Do not delete the journal to force the environment to start.
The error and environment logs provide details for diagnosis.

## Scope and limits

- Administrative hardforks support managed full localnets with masterchain
  accounts and one unsplit workchain-0 shard. Split and merged histories are not supported
- All validators must belong to the managed environment. External validators
  cannot participate in the coordinated pause and state change
- Existing queued messages remain queued. They can change the edited account
  after block production resumes
- Code or data replacement does not guarantee an immediate update of token and
  NFT metadata. These derived views can depend on later transactions
- The operation checks the new state, resumed block production and account
  indexing. It does not prove that the edited contract will behave correctly
- Changes to system contracts can affect later validator elections and network
  behavior. Recovery snapshots cover operation failures, not every delayed consequence
- The first edit on a long-running chain can take longer because Localton must
  reconstruct its state. This requires retained block and state history

## Change network configuration

Use **Network / Config** to edit blockchain configuration parameters. This page
submits changes through the configuration contract. Account actions use hardforks.
Configuration updates confirm activation in the masterchain. They do not use the
administrative hardfork workflow or its automatic recovery snapshots.
