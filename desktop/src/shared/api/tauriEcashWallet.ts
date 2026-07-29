import { invokeTauri } from "./tauri";

/**
 * Typed wrappers for the Fedimint e-cash wallet Tauri commands
 * (`desktop/src-tauri/src/commands/ecash_wallet.rs`).
 *
 * Wire shapes are snake_case exactly as the Rust structs serialize. Amounts
 * are millisatoshis (`*_msat`) everywhere; the UI layer owns sats conversion
 * (`features/ecash-wallet/lib/format.ts`).
 */

/** Wallet summary returned by `wallet_join` / `wallet_info`. */
export type EcashWalletInfo = {
  /** Hex-encoded federation id. */
  federation_id: string;
  /** Federation display name from config meta; `null` when unknown. */
  name: string | null;
  /** Bitcoin network of the federation (e.g. "bitcoin", "regtest"); `null` when unknown. */
  network: string | null;
  /** Total spendable e-cash balance in millisatoshis. */
  balance_msat: number;
};

/**
 * `wallet_status` response. When no wallet exists on this machine only
 * `state: "none"` is present. When open, `name`/`network` are OMITTED (not
 * null) when unknown — hence optional here.
 */
export type EcashWalletStatus =
  | { state: "none" }
  | {
      state: "open";
      federation_id: string;
      name?: string;
      network?: string;
      balance_msat: number;
    };

/** `wallet_spend` response: serialized out-of-band notes for the recipient. */
export type EcashWalletSpendResult = {
  /** Serialized e-cash notes — hand these to the recipient. Treat as cash. */
  notes: string;
  /**
   * ACTUAL value of the notes in msat. May exceed the requested amount when
   * exact change was unavailable (denomination overpay) — display this value,
   * not the request.
   */
  amount_msat: number;
  /** Fedimint operation id for the spend. */
  operation_id: string;
};

/** `wallet_reissue` response. */
export type EcashWalletReissueResult = {
  /**
   * FACE value of the reissued notes in msat. Mint fees are
   * federation-dependent (zero or more), so the amount credited may be less —
   * never derive a fee from these two fields; show both as-is.
   */
  amount_msat: number;
  /** Post-reissue wallet balance in msat. */
  balance_msat: number;
};

export async function walletStatus(): Promise<EcashWalletStatus> {
  return await invokeTauri<EcashWalletStatus>("wallet_status");
}

export async function walletJoin(inviteCode: string): Promise<EcashWalletInfo> {
  return await invokeTauri<EcashWalletInfo>("wallet_join", { inviteCode });
}

export async function walletBalance(): Promise<{ balance_msat: number }> {
  return await invokeTauri<{ balance_msat: number }>("wallet_balance");
}

export async function walletInfo(): Promise<EcashWalletInfo> {
  return await invokeTauri<EcashWalletInfo>("wallet_info");
}

export async function walletSpend(
  amountMsat: number,
  timeoutSecs?: number,
): Promise<EcashWalletSpendResult> {
  return await invokeTauri<EcashWalletSpendResult>("wallet_spend", {
    amountMsat,
    ...(timeoutSecs === undefined ? null : { timeoutSecs }),
  });
}

export async function walletReissue(
  notes: string,
): Promise<EcashWalletReissueResult> {
  return await invokeTauri<EcashWalletReissueResult>("wallet_reissue", {
    notes,
  });
}

export async function walletLock(): Promise<void> {
  await invokeTauri<null>("wallet_lock");
}
