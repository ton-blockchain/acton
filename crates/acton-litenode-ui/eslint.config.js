import tseslint from "typescript-eslint"
import rootConfig from "../../eslint.config.js"

export default tseslint.config(...rootConfig, {
  languageOptions: {
    parserOptions: {
      project: ["./tsconfig.json"],
      tsconfigRootDir: import.meta.dirname,
    },
  },
})
