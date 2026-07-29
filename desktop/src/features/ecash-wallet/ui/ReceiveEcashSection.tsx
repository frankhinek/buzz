import { useState } from "react";
import { Check, LoaderCircle } from "lucide-react";

import type { EcashWalletReissueResult } from "@/shared/api/tauriEcashWallet";
import { Button } from "@/shared/ui/button";
import { Textarea } from "@/shared/ui/textarea";
import {
  SettingsOptionGroup,
  SettingsOptionRow,
} from "@/features/settings/ui/SettingsOptionGroup";
import { useWalletReissueMutation } from "../hooks";
import { walletErrorText } from "../lib/errors";
import { formatSatsAmount } from "../lib/format";

/**
 * Paste-notes form for receiving out-of-band e-cash. On success shows the
 * notes' face value and the resulting balance side by side — mint fees are
 * federation-dependent (zero or more), so the two are reported as-is and no
 * fee is ever derived from them.
 */
export function ReceiveEcashSection() {
  const [notesInput, setNotesInput] = useState("");
  const [received, setReceived] = useState<EcashWalletReissueResult | null>(
    null,
  );
  const reissueMutation = useWalletReissueMutation();

  const handleReceive = () => {
    const notes = notesInput.trim();
    if (notes === "") {
      return;
    }
    setReceived(null);
    reissueMutation.mutate(notes, {
      onSuccess: (result) => {
        setReceived(result);
        setNotesInput("");
      },
    });
  };

  return (
    <SettingsOptionGroup data-testid="ecash-receive-section">
      <SettingsOptionRow className="flex-col items-stretch gap-3 py-4">
        <div>
          <p className="text-sm font-medium">Receive e-cash</p>
          <p className="text-sm text-muted-foreground">
            Paste a string of e-cash notes to add them to your wallet.
          </p>
        </div>
        <form
          className="flex flex-col gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            handleReceive();
          }}
        >
          <Textarea
            aria-label="E-cash notes"
            className="font-mono text-xs"
            data-testid="ecash-receive-notes"
            disabled={reissueMutation.isPending}
            onChange={(event) => setNotesInput(event.target.value)}
            placeholder="Paste e-cash notes"
            value={notesInput}
          />
          <Button
            className="self-start"
            data-testid="ecash-receive-button"
            disabled={reissueMutation.isPending || notesInput.trim() === ""}
            type="submit"
          >
            {reissueMutation.isPending ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" />
            ) : null}
            {reissueMutation.isPending ? "Redeeming…" : "Receive"}
          </Button>
        </form>
        {reissueMutation.isError ? (
          <p
            className="text-sm text-destructive"
            data-testid="ecash-receive-error"
          >
            {walletErrorText(reissueMutation.error, "Receiving e-cash failed.")}
          </p>
        ) : null}
        {received ? (
          <div
            className="flex items-start gap-2 rounded-lg bg-emerald-500/10 p-3 text-sm text-emerald-600 dark:text-emerald-400"
            data-testid="ecash-receive-success"
          >
            <Check aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" />
            <p>
              Received notes worth {formatSatsAmount(received.amount_msat)}
              {"; "}your balance is now{" "}
              {formatSatsAmount(received.balance_msat)}.
            </p>
          </div>
        ) : null}
      </SettingsOptionRow>
    </SettingsOptionGroup>
  );
}
