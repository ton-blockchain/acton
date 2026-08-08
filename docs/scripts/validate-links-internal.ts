import {printErrors, validateFiles} from "next-validate-link"
import {createLinkValidationConfig, getLinkValidationInput} from "./link-validation"

async function validateInternalLinks() {
  const {files, scanned} = await getLinkValidationInput()

  printErrors(
    await validateFiles(
      files,
      createLinkValidationConfig(scanned, {
        checkRelativePaths: "as-url",
        checkRelativeUrls: true,

        checkExternal: false,
      }),
    ),
    true,
  )
}

void validateInternalLinks()
