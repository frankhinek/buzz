/**
 * Pure msat <-> sats display/parse helpers for the e-cash wallet UI.
 *
 * 1 sat = 1000 msat. Balances arrive from the backend in millisatoshis; the
 * UI displays sats and only shows the fractional (sub-sat) part when it is
 * nonzero.
 */

export const MSAT_PER_SAT = 1000;

/** Group the integer digits of a non-negative integer string ("12345" -> "12,345"). */
function groupThousands(digits: string): string {
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/**
 * Format an msat amount as sats for display: thousands-grouped integer sats,
 * with the fractional msat remainder shown only when nonzero.
 *
 * 123456000 -> "123,456"
 * 123456789 -> "123,456.789"
 * 500       -> "0.5"
 */
export function formatSatsFromMsat(msat: number): string {
  const whole = Math.floor(msat / MSAT_PER_SAT);
  const remainder = msat % MSAT_PER_SAT;
  const wholeText = groupThousands(String(whole));
  if (remainder === 0) {
    return wholeText;
  }
  const fraction = String(remainder).padStart(3, "0").replace(/0+$/, "");
  return `${wholeText}.${fraction}`;
}

/** Format an msat amount with the "sats" unit, singular-aware ("1 sat"). */
export function formatSatsAmount(msat: number): string {
  const text = formatSatsFromMsat(msat);
  return `${text} ${text === "1" ? "sat" : "sats"}`;
}

/** Thousands-grouped msat value for secondary/exact displays. */
export function formatMsat(msat: number): string {
  return `${groupThousands(String(msat))} msat`;
}

export type SatsParseResult =
  | { ok: true; msat: number }
  | { ok: false; error: string };

/**
 * Parse a user-entered sats amount (optionally fractional, msat resolution)
 * into msat. Rejects empty/non-numeric input, more than 3 decimal places
 * (finer than 1 msat), zero, and unsafe magnitudes.
 */
export function parseSatsInputToMsat(input: string): SatsParseResult {
  const trimmed = input.trim().replace(/,/g, "");
  if (trimmed === "") {
    return { ok: false, error: "Enter an amount in sats." };
  }
  const match = /^(\d+)(?:\.(\d+))?$/.exec(trimmed);
  if (!match) {
    return { ok: false, error: "Enter a valid number of sats." };
  }
  const [, wholeText, fractionText] = match;
  if (fractionText !== undefined && fractionText.length > 3) {
    return {
      ok: false,
      error: "Amounts are limited to 3 decimal places (1 msat).",
    };
  }
  const whole = Number(wholeText);
  const fractionMsat =
    fractionText === undefined ? 0 : Number(fractionText.padEnd(3, "0"));
  const msat = whole * MSAT_PER_SAT + fractionMsat;
  if (!Number.isSafeInteger(msat)) {
    return { ok: false, error: "Amount is too large." };
  }
  if (msat === 0) {
    return { ok: false, error: "Amount must be greater than zero." };
  }
  return { ok: true, msat };
}

/**
 * Truncate a hex federation id for display: first 10 and last 6 chars.
 * Federation ids are not vanity-grindable identities like pubkeys, but keep
 * the full id one copy-click away wherever this is rendered.
 */
export function truncateFederationId(id: string): string {
  if (id.length <= 20) {
    return id;
  }
  return `${id.slice(0, 10)}…${id.slice(-6)}`;
}
