import { expect, test, type Page } from "@playwright/test";

import { waitForAnimations } from "../helpers/animations";
import { installMockBridge } from "../helpers/bridge";

// Paced walkthrough of the e-cash wallet for demo video capture. Skipped in
// normal smoke runs; record with:
//
//   DEMO_VIDEO=1 pnpm exec playwright test tests/e2e/ecash-wallet-demo.spec.ts --project=smoke
//
// The video lands in test-results/<test-dir>/video.webm.

const MOCK_INVITE = "fed1qtestinvitecode0000000000";
const RECEIVE_NOTES = "mocknotesv0amt21000seq77cashuAeyJtb2NrIjp0cnVlfQ";

// Demo beat length. Long enough to read each state, short enough to keep the
// full walkthrough around a minute.
const BEAT_MS = 1800;

async function beat(page: Page, ms = BEAT_MS) {
  await waitForAnimations(page);
  await page.waitForTimeout(ms);
}

test.use({
  viewport: { width: 1280, height: 800 },
  video: { mode: "on", size: { width: 1280, height: 800 } },
});

test.describe("e-cash wallet demo video", () => {
  test.skip(!process.env.DEMO_VIDEO, "demo recording only (set DEMO_VIDEO=1)");

  test("wallet walkthrough: join, balance, send, receive", async ({
    page,
  }) => {
    test.setTimeout(120_000);
    await installMockBridge(page);

    // Open Settings -> Wallet.
    await page.goto("/", { waitUntil: "domcontentloaded" });
    await beat(page, 1200);
    await page.getByTestId("open-settings").click();
    await beat(page, 900);
    await page.getByTestId("profile-popover-settings").click();
    await expect(page.getByTestId("settings-view")).toBeVisible();
    await beat(page, 900);
    await page.getByTestId("settings-nav-wallet").click();
    const card = page.getByTestId("settings-ecash-wallet");
    await expect(card).toBeVisible({ timeout: 10_000 });
    await beat(page);

    // Join a federation: type the invite code, join, land on the overview.
    const invite = card.getByTestId("ecash-invite-input");
    await invite.click();
    await invite.pressSequentially(MOCK_INVITE, { delay: 35 });
    await beat(page, 1000);
    await card.getByTestId("ecash-join-button").click();
    await expect(card.getByTestId("ecash-wallet-overview")).toBeVisible({
      timeout: 10_000,
    });
    await beat(page, 2600);

    // Send e-cash: amount in, notes dialog out.
    const amount = card.getByTestId("ecash-send-amount");
    await amount.click();
    await amount.pressSequentially("2500", { delay: 120 });
    await beat(page, 900);
    await card.getByTestId("ecash-send-button").click();
    const dialog = page.getByTestId("ecash-send-result-dialog");
    await expect(dialog).toBeVisible({ timeout: 10_000 });
    await beat(page, 2600);
    await dialog.getByTestId("ecash-copy-notes").click();
    await beat(page, 1400);
    await page.keyboard.press("Escape");
    await expect(dialog).not.toBeVisible();
    // Dwell on the debited balance.
    await expect(card.getByTestId("ecash-balance")).toContainText("120,956");
    await beat(page, 2200);

    // Receive e-cash: paste notes, watch the balance credit.
    const receive = card.getByTestId("ecash-receive-notes");
    await receive.click();
    await receive.fill(RECEIVE_NOTES);
    await beat(page, 1200);
    await card.getByTestId("ecash-receive-button").click();
    await expect(card.getByTestId("ecash-receive-success")).toBeVisible({
      timeout: 10_000,
    });
    await expect(card.getByTestId("ecash-balance")).toContainText("120,977");
    // Closing dwell on the final state.
    await beat(page, 3000);
  });
});
