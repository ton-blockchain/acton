import tseslint from "typescript-eslint"

import rootConfig from "../../eslint.config.js"

export default tseslint.config(
  {ignores: ["dist/**", "examples/counter/build/**", "examples/counter/wrappers-ts/**"]},
  ...rootConfig,
  {
    languageOptions: {
      parserOptions: {
        project: ["./tsconfig.json", "./examples/counter/tsconfig.json"],
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "unicorn/no-null": "off",
    },
    settings: {
      react: {
        version: "19.0.0",
      },
    },
  },
)
