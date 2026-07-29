import { useState } from "react";
import { Copy, LoaderCircle, Send, TriangleAlert } from "lucide-react";

import type { EcashWalletSpendResult } from "@/shared/api/tauriEcashWallet";
import { copyTextToClipboard } from "@/shared/lib/clipboard";
import { Button } from "@/shared/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import {
  SettingsOptionGroup,
  SettingsOptionRow,
} from "@/features/settings/ui/SettingsOptionGroup";
import { useWalletSpendMutation } from "../hooks";
import { walletErrorText } from "../lib/errors";
import {
  formatMsat,
  formatSatsAmount,
  parseSatsInputToMsat,
} from "../lib/format";

type SpendOutcome = {
  requestedMsat: number;
  result: EcashWalletSpendResult;
};

/** Result dialog: the notes string is the money — show it once, prominently. */
function SendResultDialog({
  onClose,
  outcome,
}: {
  onClose: () => void;
  outcome: SpendOutcome | null;
}) {
  const overpaid =
    outcome !== null && outcome.result.amount_msat > outcome.requestedMsat;

  return (
    <Dialog
      onOpenChange={(nextOpen) => {
        if (!nextOpen) onClose();
      }}
      open={outcome !== null}
    >
      <DialogContent
        className="max-w-lg"
        data-testid="ecash-send-result-dialog"
      >
        <DialogHeader>
          <DialogTitle>E-cash ready to send</DialogTitle>
          <DialogDescription>
            Hand this string to the recipient — it is the money itself.
          </DialogDescription>
        </DialogHeader>
        {outcome === null ? null : (
          <div className="space-y-4">
            <div className="space-y-1">
              <p
                className="text-2xl font-semibold tabular-nums"
                data-testid="ecash-send-actual-amount"
                title={formatMsat(outcome.result.amount_msat)}
              >
                {formatSatsAmount(outcome.result.amount_msat)}
              </p>
              {overpaid ? (
                <p
                  className="text-sm text-amber-600 dark:text-amber-400"
                  data-testid="ecash-send-overpay-note"
                >
                  The available note denominations could not make exact change,
                  so these notes are worth more than the{" "}
                  {formatSatsAmount(outcome.requestedMsat)} you requested. The
                  recipient gets the full amount shown above.
                </p>
              ) : null}
            </div>
            <div
              className="max-h-40 overflow-y-auto whitespace-pre-wrap break-all rounded-lg border border-border/70 bg-muted/30 p-3 font-mono text-xs"
              data-testid="ecash-send-notes"
            >
              {outcome.result.notes}
            </div>
            <div
              className="flex items-start gap-2 rounded-lg bg-amber-500/10 p-3 text-sm text-amber-600 dark:text-amber-400"
              data-testid="ecash-send-cash-warning"
            >
              <TriangleAlert
                aria-hidden="true"
                className="mt-0.5 h-4 w-4 shrink-0"
              />
              <p>
                Treat this string as cash: anyone who sees it can spend it, and
                it is only shown once. If it is never redeemed, your wallet
                reclaims the funds after the spend timeout.
              </p>
            </div>
            <Button
              className="w-full"
              data-testid="ecash-copy-notes"
              onClick={() => copyTextToClipboard(outcome.result.notes)}
              type="button"
              variant="outline"
            >
              <Copy aria-hidden="true" />
              Copy e-cash notes
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

/** Amount form + result dialog for out-of-band spends. */
export function SendEcashSection() {
  const [amountInput, setAmountInput] = useState("");
  const [inputError, setInputError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<SpendOutcome | null>(null);
  const spendMutation = useWalletSpendMutation();

  const handleSend = () => {
    const parsed = parseSatsInputToMsat(amountInput);
    if (!parsed.ok) {
      setInputError(parsed.error);
      return;
    }
    setInputError(null);
    spendMutation.mutate(
      { amountMsat: parsed.msat },
      {
        onSuccess: (result) => {
          setOutcome({ requestedMsat: parsed.msat, result });
          setAmountInput("");
        },
      },
    );
  };

  const errorText = inputError
    ? inputError
    : spendMutation.isError
      ? walletErrorText(spendMutation.error, "Sending e-cash failed.")
      : null;

  return (
    <SettingsOptionGroup data-testid="ecash-send-section">
      <SettingsOptionRow className="flex-col items-stretch gap-3 py-4">
        <div>
          <p className="text-sm font-medium">Send e-cash</p>
          <p className="text-sm text-muted-foreground">
            Creates a string of e-cash notes to hand to the recipient.
          </p>
        </div>
        <form
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            handleSend();
          }}
        >
          <Input
            aria-label="Amount in sats"
            className="max-w-48"
            data-testid="ecash-send-amount"
            disabled={spendMutation.isPending}
            inputMode="decimal"
            onChange={(event) => {
              setAmountInput(event.target.value);
              setInputError(null);
            }}
            placeholder="Amount in sats"
            value={amountInput}
          />
          <Button
            data-testid="ecash-send-button"
            disabled={spendMutation.isPending || amountInput.trim() === ""}
            type="submit"
          >
            {spendMutation.isPending ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" />
            ) : (
              <Send aria-hidden="true" />
            )}
            {spendMutation.isPending ? "Preparing notes…" : "Send"}
          </Button>
        </form>
        {errorText ? (
          <p
            className="text-sm text-destructive"
            data-testid="ecash-send-error"
          >
            {errorText}
          </p>
        ) : null}
      </SettingsOptionRow>
      <SendResultDialog onClose={() => setOutcome(null)} outcome={outcome} />
    </SettingsOptionGroup>
  );
}
