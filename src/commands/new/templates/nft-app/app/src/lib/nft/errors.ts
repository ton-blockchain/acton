export function getErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null) {
    const e = error as {
      message?: string;
      response?: { status?: number };
      status?: number;
    };
    const status = e.response?.status ?? e.status ?? null;
    if (status === 429 || e.message?.includes('status code 429')) {
      return 'Toncenter rate limit reached (HTTP 429). Wait a bit or add TONCENTER_MAINNET_API_KEY / TONCENTER_TESTNET_API_KEY.';
    }
  }
  if (error instanceof Error) return error.message;
  return 'Unexpected error.';
}
