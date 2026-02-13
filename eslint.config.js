import js from "@eslint/js"
import globals from "globals"
import reactPlugin from "eslint-plugin-react"
import reactHooks from "eslint-plugin-react-hooks"
import reactRefresh from "eslint-plugin-react-refresh"
import tseslint from "typescript-eslint"
import eslintPluginJsxA11y from "eslint-plugin-jsx-a11y"
import {importX} from "eslint-plugin-import-x"
import eslintConfigPrettier from "eslint-config-prettier"
import unusedImports from "eslint-plugin-unused-imports"
import functional from "eslint-plugin-functional"
import unicorn from "eslint-plugin-unicorn"
import * as tsResolver from "eslint-import-resolver-typescript"

export const baseConfig = tseslint.config(
  {
    ignores: ["**/dist/**", "**/build/**", "**/node_modules/**"],
  },
  {
    extends: [
      js.configs.recommended,
      ...tseslint.configs.recommendedTypeChecked,
      importX.flatConfigs.recommended,
      importX.flatConfigs.typescript,
      unicorn.configs["flat/recommended"],
    ],
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
      parser: tseslint.parser,
      parserOptions: {
        project: ["./tsconfig.json"],
        tsconfigRootDir: import.meta.dirname,
        ecmaFeatures: {jsx: true},
      },
    },
    plugins: {
      "@typescript-eslint": tseslint.plugin,
      react: reactPlugin,
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
      "jsx-a11y": eslintPluginJsxA11y,
      "import-x": importX,
      "@unused-imports": unusedImports,
      functional: functional,
    },
    rules: {
      ...reactPlugin.configs.recommended.rules,
      ...reactPlugin.configs["jsx-runtime"].rules,
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": ["warn", {allowConstantExport: true}],
      "import-x/no-unresolved": "off",
      "import-x/default": "off",
      "react/prop-types": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
        },
      ],
      ...eslintPluginJsxA11y.configs.recommended.rules,
      "import-x/order": [
        "warn",
        {
          groups: ["builtin", "external", "internal", "parent", "sibling", "index"],
          "newlines-between": "always-and-inside-groups",
        },
      ],
      "@unused-imports/no-unused-imports": "error",
      "jsx-a11y/no-autofocus": "off",
      "functional/type-declaration-immutability": [
        "error",
        {
          rules: [
            {
              identifiers: ".+",
              immutability: "ReadonlyShallow",
              comparator: "AtLeast",
            },
          ],
        },
      ],
      "unicorn/filename-case": "off",
      "unicorn/prevent-abbreviations": "off",
      "unicorn/no-array-reduce": "off",
      "unicorn/consistent-function-scoping": "off",
    },
    settings: {
      react: {
        version: "detect",
      },
      "import-x/resolver": {
        name: "tsResolver",
        resolver: tsResolver,
      },
    },
  },
  eslintConfigPrettier,
)

export default baseConfig
