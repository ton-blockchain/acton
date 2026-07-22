# UI packages

The `packages` workspace contains the web applications, shared UI libraries, and browser
tests used by Acton.

| Package | Responsibility |
| --- | --- |
| [`@acton/ui`](./ui) | Shared design tokens, themes, generic components, and low-level UI helpers. It must not depend on Acton application or TON transaction logic. |
| [`@acton/transaction-ui`](./transaction-ui) | Reusable TON transaction presentation, including transaction details, phases, messages, and code or cell inspection. |
| [`@acton/localnet-ui`](./localnet-ui) | The web interface served by an Acton local node. It owns localnet controls and the reusable explorer implementation. |
| [`@acton/explorer-ui`](./explorer-ui) | The standalone explorer application deployed at [actonscan.com](https://actonscan.com) and its deployment integration. It reuses the explorer features from `@acton/localnet-ui`. |
| [`@acton/test-ui`](./test-ui) | The report application opened by `acton test --ui`, including test results, logs, traces, coverage, and gas profiles. |
| [`@acton/ui-gallery`](./ui-gallery) | The visual catalog used to review and document `@acton/ui` components and their states. |
| [`@acton/ui-e2e`](./ui-e2e) | Shared Playwright scenarios and fixtures for end-to-end and visual-regression coverage of the UI applications. |

Keep generic primitives in `@acton/ui`, transaction-specific presentation in
`@acton/transaction-ui`, and product behavior in the application that owns it.
