import { expect, test, type Page } from "@playwright/test";

import { waitForAnimations } from "../helpers/animations";
import { installMockBridge } from "../helpers/bridge";

const SHOTS = "test-results/wallet-screenshots";

// Mock bridge constants (e2eBridge.ts mockWalletState): joining any `fed1…`
// invite opens "Test Federation" on regtest with 123,456,000 msat.
const MOCK_INVITE = "fed1qtestinvitecode0000000000";
const INITIAL_BALANCE_SATS = "123,456";

// Notes in the mock's own format: face value 21,000 msat (21 sats).
const RECEIVE_NOTES = "mocknotesv0amt21000seq77cashuAeyJtb2NrIjp0cnVlfQ";

// Navigate to the Wallet settings panel.
async function openWalletSettings(page: Page) {
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.getByTestId("open-settings").click();
  await page.getByTestId("profile-popover-settings").click();
  await expect(page.getByTestId("settings-view")).toBeVisible();
  await page.getByTestId("settings-nav-wallet").click();
  const card = page.getByTestId("settings-ecash-wallet");
  await expect(card).toBeVisible({ timeout: 10_000 });
  return card;
}

// Join the mock federation and wait for the open-wallet overview.
async function joinMockFederation(card: ReturnType<Page["locator"]>) {
  await card.getByTestId("ecash-invite-input").fill(MOCK_INVITE);
  await card.getByTestId("ecash-join-button").click();
  await expect(card.getByTestId("ecash-wallet-overview")).toBeVisible({
    timeout: 10_000,
  });
}

test.describe("e-cash wallet screenshots", () => {
  test.use({ viewport: { width: 1280, height: 900 } });

  test.beforeEach(async ({ page }) => {
    page.on("pageerror", (err) => {
      console.error(
        "PAGE ERROR:",
        err.message,
        err.stack?.split("\n").slice(0, 5).join("\n"),
      );
    });
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        console.error("CONSOLE ERROR:", msg.text().slice(0, 500));
      }
    });
  });

  test("01 — no wallet: join federation form", async ({ page }) => {
    await installMockBridge(page);
    const card = await openWalletSettings(page);

    await expect(card.getByTestId("ecash-join-form")).toBeVisible();
    // Join is disabled until an invite code is entered.
    await expect(card.getByTestId("ecash-join-button")).toBeDisabled();
    await card.getByTestId("ecash-invite-input").fill(MOCK_INVITE);
    await expect(card.getByTestId("ecash-join-button")).toBeEnabled();
    // Clear again so the screenshot shows the pristine empty state.
    await card.getByTestId("ecash-invite-input").fill("");

    await waitForAnimations(page);
    await card.screenshot({ path: `${SHOTS}/01-no-wallet-join.png` });
  });

  test("02 — join transitions to the open wallet with balance", async ({
    page,
  }) => {
    await installMockBridge(page);
    const card = await openWalletSettings(page);

    await joinMockFederation(card);

    // Balance in sats (fractional part hidden when zero), exact msat secondary.
    await expect(card.getByTestId("ecash-balance")).toContainText(
      INITIAL_BALANCE_SATS,
    );
    await expect(card.getByTestId("ecash-balance-msat")).toHaveText(
      "123,456,000 msat",
    );
    await expect(card.getByTestId("ecash-federation-name")).toHaveText(
      "Test Federation",
    );
    // Non-mainnet network is badged.
    await expect(card.getByTestId("ecash-network-badge")).toHaveText("regtest");
    // Federation id renders truncated with the full id as tooltip.
    await expect(card.getByTestId("ecash-federation-id")).toContainText("…");
    await expect(card.getByTestId("ecash-federation-id")).toHaveAttribute(
      "title",
      /^f00dbabe/,
    );

    await waitForAnimations(page);
    await card.screenshot({ path: `${SHOTS}/02-open-wallet-balance.png` });
  });

  test("03 — send shows the notes result dialog and debits the balance", async ({
    page,
  }) => {
    await installMockBridge(page);
    const card = await openWalletSettings(page);
    await joinMockFederation(card);

    await card.getByTestId("ecash-send-amount").fill("1000");
    await card.getByTestId("ecash-send-button").click();

    const dialog = page.getByTestId("ecash-send-result-dialog");
    await expect(dialog).toBeVisible({ timeout: 10_000 });
    // The dialog reports the ACTUAL note value (mock: exact) and the notes.
    await expect(dialog.getByTestId("ecash-send-actual-amount")).toHaveText(
      "1,000 sats",
    );
    await expect(dialog.getByTestId("ecash-send-notes")).toContainText(
      "mocknotesv0amt1000000seq1",
    );
    await expect(dialog.getByTestId("ecash-send-cash-warning")).toContainText(
      /treat this string as cash/i,
    );
    await expect(dialog.getByTestId("ecash-copy-notes")).toBeVisible();

    await waitForAnimations(page);
    await dialog.screenshot({ path: `${SHOTS}/03-send-notes-dialog.png` });

    // Closing the dialog reveals the debited balance (123,456 - 1,000 sats).
    await page.keyboard.press("Escape");
    await expect(dialog).not.toBeVisible();
    await expect(card.getByTestId("ecash-balance")).toContainText("122,456");
  });

  test("04 — receive credits the balance and reports face value", async ({
    page,
  }) => {
    await installMockBridge(page);
    const card = await openWalletSettings(page);
    await joinMockFederation(card);

    // Invalid notes surface the backend error inline.
    await card.getByTestId("ecash-receive-notes").fill("not-real-notes");
    await card.getByTestId("ecash-receive-button").click();
    await expect(card.getByTestId("ecash-receive-error")).toContainText(
      "invalid e-cash notes",
    );

    // Valid notes: face value 21 sats, balance 123,456 + 21 = 123,477 sats.
    await card.getByTestId("ecash-receive-notes").fill(RECEIVE_NOTES);
    await card.getByTestId("ecash-receive-button").click();
    const success = card.getByTestId("ecash-receive-success");
    await expect(success).toBeVisible({ timeout: 10_000 });
    await expect(success).toContainText(
      "Received notes worth 21 sats; your balance is now 123,477 sats.",
    );
    await expect(card.getByTestId("ecash-balance")).toContainText("123,477");

    await waitForAnimations(page);
    await card.screenshot({ path: `${SHOTS}/04-receive-success.png` });
  });
});
