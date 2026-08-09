# Packages

The `packages` workspace contains shared libraries, web applications, UI components, and
browser tests used by Acton.

| Package                                         | Responsibility                                                                                                                                                                                             |
|-------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| [`@acton/address-registry`](./address-registry) | Unified, provenance-aware TON address and asset metadata. It normalizes community-maintained labels, verification and safety signals without treating self-declared on-chain metadata as trusted identity. |
| [`@acton/ui`](./ui)                             | Shared design tokens, themes, generic components, and low-level UI helpers. It must not depend on Acton application or TON transaction logic.                                                              |
| [`@acton/transaction-ui`](./transaction-ui)     | Reusable TON transaction presentation, including transaction details, phases, messages, and code or cell inspection.                                                                                       |
| [`@acton/explorer-core`](./explorer-core)       | Reusable TON explorer client, pages, metadata registries, cell inspector, and retrace UI shared by Acton Studio and Actonscan.                                                                              |
| [`@acton/explorer-ui`](./explorer-ui)           | The standalone Actonscan application and deployment integration. It hosts the explorer features from `@acton/explorer-core`.                                                                               |
| [`@acton/verifier-ui`](./verifier-ui)           | The frontend embedded into the Acton source verifier service, including contract verification flows and verification status presentation.                                                                  |
| [`@acton/test-ui`](./test-ui)                   | The report application opened by `acton test --ui`, including test results, logs, traces, coverage, and gas profiles.                                                                                      |
| [`@acton/ui-gallery`](./ui-gallery)             | The visual catalog used to review and document `@acton/ui` components and their states.                                                                                                                    |
| [`@acton/ui-e2e`](./ui-e2e)                     | Shared Playwright scenarios and fixtures for end-to-end and visual-regression coverage of the UI applications.                                                                                             |

Keep generic primitives in `@acton/ui`, transaction-specific presentation in
`@acton/transaction-ui`, and product behavior in the application that owns it.
