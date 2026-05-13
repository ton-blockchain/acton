export const baseUrl =
  process.env.GITHUB_ACTIONS === "true" || process.env.GITHUB_PAGES === "true"
    ? "https://ton-blockchain.github.io/acton"
    : "http://localhost:3000"
