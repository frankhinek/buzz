/**
 * Render an unknown thrown value as user-facing text. The wallet backend
 * returns `Err(String)` with user-readable messages, which the Tauri invoke
 * layer surfaces as `Error` instances — show them verbatim.
 */
export function walletErrorText(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }
  if (typeof error === "string" && error) {
    return error;
  }
  return fallback;
}
