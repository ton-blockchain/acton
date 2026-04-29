import { type ExternalLinkResult, printErrors, validateFiles } from 'next-validate-link';
import { createLinkValidationConfig, getLinkValidationInput } from './link-validation';

const externalLinkTimeoutMs = 15_000;

async function validateExternalLinks() {
  const { files, scanned } = await getLinkValidationInput();

  printErrors(
    await validateFiles(
      files,
      createLinkValidationConfig(scanned, {
        checkRelativePaths: false,
        checkRelativeUrls: false,

        whitelist,
        checkExternal: {
          validate: validateExternalUrl,
        },
      }),
    ),
    true,
  );
}

function whitelist(url: string): boolean {
  if (url.startsWith('https://github.com/ton-blockchain/acton')) {
    return true;
  }

  // TODO(danil42russia): remove after release
  if (url.startsWith('https://github.com/ton-blockchain/setup-acton')) {
    return true;
  }

  // TODO(danil42russia): remove after release
  if (url.startsWith('https://github.com/i582/acton-public/releases')) {
    return true;
  }

  return false;
}

async function validateExternalUrl(url: URL): Promise<ExternalLinkResult> {
  try {
    return await checkExternalUrl(url);
  } catch (error: unknown) {
    return {
      success: false,
      message: error instanceof Error ? error.message : String(error),
    };
  }
}

async function checkExternalUrl(url: URL): Promise<ExternalLinkResult> {
  const response = await fetch(url, {
    method: 'GET',
    redirect: 'manual',
    signal: AbortSignal.timeout(externalLinkTimeoutMs),
  });

  await response.body?.cancel();

  const status = response.status;
  if (status === 200) {
    return { success: true };
  }

  if (status >= 300 && status < 400) {
    return {
      success: false,
      message: `redirected to '${response.headers.get('location')}'`,
    };
  }

  if (status >= 400 && status < 500) {
    let message = `client error ${status}`;
    switch (status) {
      case 404:
        message = `not found ${url}`;
        break;
    }

    return {
      success: false,
      message: message,
    };
  }

  return {
    success: false,
    message: `unknown response code ${status}`,
  };
}

void validateExternalLinks();
