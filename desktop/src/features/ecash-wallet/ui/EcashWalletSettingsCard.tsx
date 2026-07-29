import { useState } from "react";
import { Copy, LoaderCircle, RefreshCw } from "lucide-react";

import { copyTextToClipboard } from "@/shared/lib/clipboard";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import {
  SettingsOptionGroup,
  SettingsOptionRow,
} from "@/features/settings/ui/SettingsOptionGroup";
import { SettingsSectionHeader } from "@/features/settings/ui/SettingsSectionHeader";
import { useWalletJoinMutation, useWalletStatusQuery } from "../hooks";
import { walletErrorText } from "../lib/errors";
import {
  formatMsat,
  formatSatsFromMsat,
  truncateFederationId,
} from "../lib/format";
import { ReceiveEcashSection } from "./ReceiveEcashSection";
import { SendEcashSection } from "./SendEcashSection";

/** Networks where funds are real money; everything else gets a warning badge. */
function isMainnet(network: string): boolean {
  return network === "bitcoin" || network === "mainnet";
}

/** Invite-code form shown while no wallet exists on this machine. */
function JoinFederationForm() {
  const [inviteCode, setInviteCode] = useState("");
  const joinMutation = useWalletJoinMutation();

  return (
    <SettingsOptionGroup data-testid="ecash-join-form">
      <SettingsOptionRow className="flex-col items-stretch gap-3 py-4">
        <div>
          <p className="text-sm font-medium">Join a federation</p>
          <p className="text-sm text-muted-foreground">
            E-cash lives in a Fedimint federation. Paste a federation invite
            code to create this machine&apos;s wallet — you can hold, send, and
            receive e-cash once joined.
          </p>
        </div>
        <form
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            const trimmed = inviteCode.trim();
            if (trimmed === "" || joinMutation.isPending) {
              return;
            }
            joinMutation.mutate(trimmed);
          }}
        >
          <Input
            aria-label="Federation invite code"
            data-testid="ecash-invite-input"
            disabled={joinMutation.isPending}
            onChange={(event) => setInviteCode(event.target.value)}
            placeholder="fed1…"
            value={inviteCode}
          />
          <Button
            data-testid="ecash-join-button"
            disabled={joinMutation.isPending || inviteCode.trim() === ""}
            type="submit"
          >
            {joinMutation.isPending ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" />
            ) : null}
            {joinMutation.isPending ? "Joining…" : "Join"}
          </Button>
        </form>
        {joinMutation.isPending ? (
          <p className="text-sm text-muted-foreground">
            Downloading the federation configuration — this can take a few
            seconds.
          </p>
        ) : null}
        {joinMutation.isError ? (
          <p
            className="text-sm text-destructive"
            data-testid="ecash-join-error"
          >
            {walletErrorText(
              joinMutation.error,
              "Joining the federation failed.",
            )}
          </p>
        ) : null}
      </SettingsOptionRow>
    </SettingsOptionGroup>
  );
}

/** Balance + federation identity for the open wallet. */
function WalletOverview({
  balanceMsat,
  federationId,
  name,
  network,
}: {
  balanceMsat: number;
  federationId: string;
  name?: string;
  network?: string;
}) {
  return (
    <SettingsOptionGroup data-testid="ecash-wallet-overview">
      <SettingsOptionRow className="py-4">
        <div className="min-w-0">
          <p className="text-sm text-muted-foreground">Balance</p>
          <p
            className="text-2xl font-semibold tabular-nums"
            data-testid="ecash-balance"
            title={formatMsat(balanceMsat)}
          >
            {formatSatsFromMsat(balanceMsat)}{" "}
            <span className="text-base font-normal text-muted-foreground">
              sats
            </span>
          </p>
          <p
            className="text-2xs text-muted-foreground"
            data-testid="ecash-balance-msat"
          >
            {formatMsat(balanceMsat)}
          </p>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1">
          <p
            className="text-sm font-medium"
            data-testid="ecash-federation-name"
          >
            {name ?? "Unnamed federation"}
          </p>
          {network ? (
            <Badge
              data-testid="ecash-network-badge"
              variant={isMainnet(network) ? "secondary" : "warning"}
            >
              {network}
            </Badge>
          ) : null}
        </div>
      </SettingsOptionRow>
      <SettingsOptionRow className="min-h-12 border-t border-border/40 py-2">
        <p className="text-sm text-muted-foreground">Federation ID</p>
        <div className="flex items-center gap-1">
          <span
            className="font-mono text-xs text-muted-foreground"
            data-testid="ecash-federation-id"
            title={federationId}
          >
            {truncateFederationId(federationId)}
          </span>
          <Button
            aria-label="Copy federation ID"
            data-testid="ecash-copy-federation-id"
            onClick={() => copyTextToClipboard(federationId)}
            size="icon-xs"
            type="button"
            variant="ghost"
          >
            <Copy aria-hidden="true" />
          </Button>
        </div>
      </SettingsOptionRow>
    </SettingsOptionGroup>
  );
}

/**
 * Settings card for the app-wide Fedimint e-cash wallet: join a federation
 * when none exists, otherwise balance overview plus out-of-band send/receive.
 */
export function EcashWalletSettingsCard() {
  const statusQuery = useWalletStatusQuery();

  return (
    <section className="min-w-0" data-testid="settings-ecash-wallet">
      <SettingsSectionHeader
        title="Wallet"
        description="Hold and pay with e-cash from a Fedimint federation. The wallet belongs to this machine and works across all your communities."
      />
      {statusQuery.isPending ? (
        <div
          className="flex items-center gap-2 text-sm text-muted-foreground"
          data-testid="ecash-status-loading"
        >
          <LoaderCircle aria-hidden="true" className="h-4 w-4 animate-spin" />
          Checking wallet…
        </div>
      ) : statusQuery.isError ? (
        <div className="space-y-2" data-testid="ecash-status-error">
          <p className="text-sm text-destructive">
            {walletErrorText(
              statusQuery.error,
              "The wallet could not be checked.",
            )}
          </p>
          <Button
            onClick={() => void statusQuery.refetch()}
            size="sm"
            type="button"
            variant="outline"
          >
            <RefreshCw aria-hidden="true" />
            Try again
          </Button>
        </div>
      ) : statusQuery.data.state === "none" ? (
        <JoinFederationForm />
      ) : (
        <div className="space-y-6">
          <WalletOverview
            balanceMsat={statusQuery.data.balance_msat}
            federationId={statusQuery.data.federation_id}
            name={statusQuery.data.name}
            network={statusQuery.data.network}
          />
          <SendEcashSection />
          <ReceiveEcashSection />
        </div>
      )}
    </section>
  );
}
