import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  walletJoin,
  walletReissue,
  walletSpend,
  walletStatus,
  type EcashWalletStatus,
} from "@/shared/api/tauriEcashWallet";

/**
 * React-query hooks for the e-cash wallet. The wallet is app-wide (Phase 2:
 * one wallet per installation, not per community), so the status key is
 * stable and every balance-changing mutation invalidates it.
 */

export const ecashWalletStatusQueryKey = ["ecash-wallet-status"] as const;

export function useWalletStatusQuery() {
  return useQuery<EcashWalletStatus>({
    queryKey: ecashWalletStatusQueryKey,
    queryFn: walletStatus,
    // Balance changes only through this UI's own mutations (which invalidate
    // explicitly); a slow poll backstops external changes such as a spend
    // timing out and being reclaimed by the wallet.
    staleTime: 30_000,
    refetchInterval: 60_000,
    retry: false,
  });
}

export function useWalletJoinMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (inviteCode: string) => walletJoin(inviteCode),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: ecashWalletStatusQueryKey,
      });
    },
  });
}

export function useWalletSpendMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      amountMsat,
      timeoutSecs,
    }: {
      amountMsat: number;
      timeoutSecs?: number;
    }) => walletSpend(amountMsat, timeoutSecs),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: ecashWalletStatusQueryKey,
      });
    },
  });
}

export function useWalletReissueMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (notes: string) => walletReissue(notes),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: ecashWalletStatusQueryKey,
      });
    },
  });
}
