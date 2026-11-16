import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";
import { fileURLToPath } from "node:url";
import { readFileSync } from "node:fs";
import { money } from "../src/api";

const fixture = (name: string) =>
  fileURLToPath(
    new URL(`../../backend/tests/fixtures/${name}`, import.meta.url),
  );

async function createAccount(page: Page, name = "Everyday EUR") {
  await page.goto("/");
  await expect(page.getByText("Local API connected")).toBeAttached();
  // Each test creates a distinct account; the API database is disposable.
  if (
    await page.getByRole("button", { name: "Add your first account" }).count()
  ) {
    await page.getByRole("button", { name: "Add your first account" }).click();
  } else {
    await page
      .getByRole("button", { name: "Add account", exact: true })
      .click();
  }
  await page.getByLabel("Account name").fill(name);
  await page
    .getByRole("button", { name: "Create account", exact: true })
    .click();
  await expect(page.getByRole("heading", { level: 1, name })).toBeVisible();
  await expect(page.locator(".balance-value")).toHaveText("Not available");
}

async function upload(page: Page, path: string) {
  await page
    .getByRole("button", { name: "Import statement", exact: true })
    .click();
  await page.getByLabel("Choose Revolut CSV").setInputFiles(path);
  await page
    .getByRole("button", { name: "Preview statement", exact: true })
    .click();
}

async function confirmImport(page: Page) {
  await page
    .getByLabel(
      "I approve these row decisions and the selected balance evidence.",
    )
    .check();
  await page
    .getByRole("button", { name: "Confirm import", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Statement saved", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Back to account", exact: true })
    .click();
}

test("keeps every money digit and distinguishes unknown from zero", () => {
  expect(money("99999999999999999999.12345678", "CHF")).toBe(
    "99,999,999,999,999,999,999.12345678 CHF",
  );
  expect(money("-0.00", "EUR")).toBe("−0.00 EUR");
  expect(money("0", "EUR")).toBe("0.00 EUR");
  expect(money(null, "EUR")).toBe("Not available");
});

test("confirms timed references and reviews a ghost through late imports to zero", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await createAccount(page, "Checkpoint EUR");
  async function timedBalance(amount: string, date: string) {
    await page.getByRole("button", { name: "Confirm balance", exact: true }).click();
    await page.getByLabel("Actual balance (EUR)").fill(amount);
    await page.getByLabel("Balance date", { exact: true }).fill(date);
    await page.getByLabel("Use this balance as a timed reference checkpoint").check();
    await page.getByLabel("Checkpoint time", { exact: true }).fill("00:00:00");
    await page.getByLabel("Checkpoint UTC offset", { exact: true }).fill("+00:00");
    await page.getByLabel("I confirm this is the account balance for the stated date.").check();
    await page.getByRole("button", { name: "Save confirmed balance", exact: true }).click();
    await expect(page.locator(".balance-value")).toContainText(amount);
  }
  await timedBalance("100", "2025-01-01");
  await page.getByLabel("Matapan default timezone", { exact: true }).fill("UTC");
  await page.getByRole("button", { name: "Save Matapan timezone", exact: true }).click();
  await expect(page.getByText("Saved: UTC", { exact: false })).toBeVisible();
  await timedBalance("190", "2025-02-01");
  const period = page.locator(".reconciliation-period");
  await expect(period).toHaveCount(1);
  await expect(period.locator("dd").last()).toHaveText("90.00 EUR");
  async function acceptGhost() {
    await period.getByLabel("Reason for adjustment decision").fill("Synthetic missing statement data");
    await period.getByLabel("I reviewed these checkpoints, movements, and the proposed ghost decision.").check();
    await period.getByRole("button", { name: "Confirm proposed ghost amount", exact: true }).click();
    await expect(period.getByText("reconciled", { exact: true })).toBeVisible();
  }
  await acceptGhost();
  async function lateImport(amount: string, day: string) {
    const csv = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n" +
      `Topup,Current,2025-01-${day} 12:00:00,2025-01-${day} 12:00:00,Synthetic late movement,${amount},0,EUR,COMPLETED,\n`;
    await page.getByRole("button", { name: "Import statement", exact: true }).click();
    await page.getByLabel("Choose Revolut CSV").setInputFiles({ name: `synthetic-${day}.csv`, mimeType: "text/csv", buffer: Buffer.from(csv) });
    await page.getByRole("button", { name: "Preview statement", exact: true }).click();
    await confirmImport(page);
  }
  await lateImport("60", "10");
  await expect(period.getByText("needs review", { exact: true })).toBeVisible();
  await expect(period.locator("dd").last()).toHaveText("30.00 EUR");
  await acceptGhost();
  await lateImport("30", "20");
  await expect(period.getByText("needs review", { exact: true })).toBeVisible();
  await expect(period.locator("dd").last()).toHaveText("0.00 EUR");
  await acceptGhost();
  await expect(period.getByRole("button", { name: "Retire approved ghost" })).toHaveCount(0);
  await expect(period.getByText("Adjustment audit history (3)")).toBeVisible();
  await expect(page.locator(".activity-panel tbody tr")).toHaveCount(2);
  await page.getByLabel("Calculate balance at", { exact: true }).fill("2025-02-02T00:00");
  await page.getByLabel("Calculation UTC offset", { exact: true }).fill("+00:00");
  await page.getByRole("button", { name: "Calculate from checkpoint" }).click();
  await expect(page.locator(".projection-result .amount")).toHaveText("190.00 EUR · calculated");
  expect(errors).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});

test("imports, reviews overlaps, preserves duplicate provenance, and confirms a dated balance", async ({
  page,
}) => {
  const consoleErrors: string[] = [];
  page.on("pageerror", (error) => consoleErrors.push(error.message));
  await createAccount(page);
  await upload(page, fixture("revolut_mixed_products.csv"));
  await expect(
    page.getByRole("heading", { name: "Review before you import" }),
  ).toBeVisible();
  await expect(
    page.getByText("All required row decisions are complete."),
  ).toBeVisible();
  // Preview must not accept financial data before the final confirmation.
  await expect(page.locator(".balance-value")).toHaveText("Not available");
  await expect(
    page.getByRole("button", { name: "Add account", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Confirm import", exact: true }),
  ).toBeDisabled();
  await confirmImport(page);
  await expect(page.locator(".balance-value")).toContainText("86.75");
  await expect(page.locator(".status-badge")).toHaveText("Source reported");
  await expect(page.locator(".activity-panel tbody tr")).toHaveCount(3);

  await upload(page, fixture("revolut_mixed_products.csv"));
  await expect(
    page.getByRole("heading", { name: "Statement saved", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Back to account" }).click();
  await expect(page.locator(".activity-panel tbody tr")).toHaveCount(3);

  await upload(page, fixture("revolut_overlap.csv"));
  await expect(
    page.getByText("2 rows need your decision before import."),
  ).toBeVisible();
  await page
    .getByLabel("Decision for row 2")
    .selectOption({ label: "Link to existing match 1" });
  await page
    .getByLabel("Decision for row 3")
    .selectOption({ label: "Link to existing match 1" });
  await confirmImport(page);
  await expect(page.locator(".activity-panel tbody tr")).toHaveCount(4);
  await expect(page.getByText("2 source rows", { exact: true })).toHaveCount(2);
  await expect(page.locator(".balance-value")).toContainText("106.75");

  await page
    .getByRole("button", { name: "Confirm balance", exact: true })
    .click();
  await page
    .getByLabel("Actual balance (EUR)")
    .fill("99999999999999999999.12345678");
  await page.getByLabel("Balance date", { exact: true }).fill("2026-10-13");
  await page
    .getByLabel("Note (optional)")
    .fill("Synthetic confirmation with incomplete history");
  await page
    .getByLabel("I confirm this is the account balance for the stated date.")
    .check();
  await page.getByRole("button", { name: "Save confirmed balance" }).click();
  await expect(page.locator(".balance-value")).toContainText(
    "99,999,999,999,999,999,999.12345678",
  );
  await expect(page.locator(".status-badge")).toHaveText("Confirmed by you");
  await expect(
    page.getByRole("heading", { name: "History coverage unverified" }),
  ).toBeVisible();
  await page.getByLabel("Report as of").fill("2024-12-31");
  await expect(page.locator(".balance-value")).toHaveText("Not available");
  await page.getByLabel("Report as of").fill("2026-10-13");
  await expect(page.locator(".status-badge")).toHaveText("Confirmed by you");
  await page.reload();
  await page
    .getByRole("navigation", { name: "Accounts" })
    .getByRole("button")
    .filter({ hasText: "Everyday EUR" })
    .last()
    .click();
  await page.getByLabel("Report as of").fill("2026-10-13");
  await expect(page.locator(".balance-value")).toContainText(
    "99,999,999,999,999,999,999.12345678",
  );
  // Long exact amounts and tables must not overflow the page at mobile sizes.
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  expect(consoleErrors).toEqual([]);
  await page.screenshot({
    path: `test-results/account-${test.info().project.name}.png`,
    fullPage: true,
  });
});

test("requires review of invalid rows, supports cancellation, and preserves balance conflicts", async ({
  page,
}) => {
  await createAccount(page, "Review EUR");
  await page.getByRole("button", { name: "Import statement" }).click();
  const content =
    "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n" +
    "Topup,Current,2025-01-02 09:00:00,2025-01-02 09:00:00,Synthetic valid,25.00,0.00,EUR,COMPLETED,25.00\n" +
    "Card Payment,Current,2025-01-03 10:00:00,,Synthetic missing date,-2.00,0.00,EUR,COMPLETED,\n";
  await page
    .getByLabel("Choose Revolut CSV")
    .setInputFiles({
      name: "synthetic-invalid.csv",
      mimeType: "text/csv",
      buffer: Buffer.from(content),
    });
  await page.getByRole("button", { name: "Preview statement" }).click();
  await expect(
    page.getByText("1 row needs your decision before import."),
  ).toBeVisible();
  await page.getByLabel("Decision for row 3").selectOption("skip");
  await page.getByRole("button", { name: "Cancel import" }).click();
  await expect(page.locator(".balance-value")).toHaveText("Not available");
  await expect(page.locator(".activity-panel tbody tr")).toHaveCount(0);

  for (const value of ["10.00", "20.00"]) {
    await page
      .getByRole("button", { name: "Confirm balance", exact: true })
      .click();
    await page.getByLabel("Actual balance (EUR)").fill(value);
    await page.getByLabel("Balance date", { exact: true }).fill("2026-10-13");
    await page
      .getByLabel("I confirm this is the account balance for the stated date.")
      .check();
    await page.getByRole("button", { name: "Save confirmed balance" }).click();
    await expect(
      page.getByRole("heading", { name: "Confirm the actual balance" }),
    ).toHaveCount(0);
  }
  await expect(page.locator(".status-badge")).toHaveText(
    "Conflicting evidence",
  );
  await expect(page.locator(".balance-value")).toHaveText("Not available");
  await expect(page.locator(".balance-list li")).toHaveCount(2);
});

test("explains API failure and recovers on retry", async ({ page }) => {
  await page.route("**/api/v1/accounts?*", (route) =>
    route.fulfill({
      status: 503,
      contentType: "application/json",
      body: '{"detail":"Synthetic unavailable API"}',
    }),
  );
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText(
    "Synthetic unavailable API",
  );
  await page.unroute("**/api/v1/accounts?*");
  await page.getByRole("button", { name: "Retry connection" }).click();
  await expect(page.getByText("Local API connected")).toBeAttached();
});

test("refreshes changed matches and requires new review before committing", async ({
  page,
}) => {
  await createAccount(page, "Concurrent EUR");
  await upload(page, fixture("revolut_mixed_products.csv"));
  const accounts = (await (
    await page.request.get("/api/v1/accounts")
  ).json()) as { id: string; name: string }[];
  const account = accounts
    .filter((item) => item.name === "Concurrent EUR")
    .at(-1)!;
  const contents =
    readFileSync(fixture("revolut_mixed_products.csv"), "utf8").trimEnd() +
    "\nTopup,Current,2025-01-06 12:00:00,2025-01-06 12:00:00,Synthetic parallel deposit,20.00,0.00,EUR,COMPLETED,106.75\n";
  // A second client accepts a different overlapping file after the UI preview.
  const second = await page.request.post("/api/v1/imports/preview", {
    multipart: {
      account_id: account.id,
      file: {
        name: "synthetic-parallel.csv",
        mimeType: "text/csv",
        buffer: Buffer.from(contents),
      },
    },
  });
  expect(second.ok()).toBe(true);
  const secondPreview = (await second.json()) as { id: string };
  const committed = await page.request.post(
    `/api/v1/imports/${secondPreview.id}/commit`,
    { data: {} },
  );
  expect(committed.ok()).toBe(true);
  await page
    .getByLabel(
      "I approve these row decisions and the selected balance evidence.",
    )
    .check();
  await page
    .getByRole("button", { name: "Confirm import", exact: true })
    .click();
  await expect(
    page.getByText("3 rows need your decision before import."),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Confirm import", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByLabel(
      "I approve these row decisions and the selected balance evidence.",
    ),
  ).not.toBeChecked();
  for (const row of [4, 5, 6])
    await page
      .getByLabel(`Decision for row ${row}`)
      .selectOption({ label: "Link to existing match 1" });
  await confirmImport(page);
  await expect(page.locator(".activity-panel tbody tr")).toHaveCount(4);
});

test("blocks expired previews and supports a balance after an import without balance evidence", async ({
  page,
}) => {
  await createAccount(page, "Partial EUR");
  const content =
    "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n" +
    "Topup,Current,2025-01-02 09:00:00,2025-01-02 09:00:00,Synthetic partial deposit,25.00,0.00,EUR,COMPLETED,\n";
  const openPreview = async () => {
    await page.getByRole("button", { name: "Import statement" }).click();
    await page
      .getByLabel("Choose Revolut CSV")
      .setInputFiles({
        name: "synthetic-no-balance.csv",
        mimeType: "text/csv",
        buffer: Buffer.from(content),
      });
    await page.getByRole("button", { name: "Preview statement" }).click();
    await expect(
      page.getByText("No trustworthy closing balance was established.", {
        exact: false,
      }),
    ).toBeVisible();
  };
  await page.clock.install();
  await openPreview();
  await page.clock.setSystemTime(new Date(Date.now() + 31 * 60_000));
  await page.clock.runFor(10_000);
  await expect(page.getByRole("alert")).toContainText(
    "This preview has expired",
  );
  await expect(
    page.getByRole("button", { name: "Confirm import", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Close expired preview" }).click();
  await page.clock.setSystemTime(new Date());
  await openPreview();
  await confirmImport(page);
  await expect(page.locator(".balance-value")).toHaveText("Not available");
  await expect(page.locator(".activity-panel tbody tr")).toHaveCount(1);
  await page
    .getByRole("button", { name: "Confirm balance", exact: true })
    .click();
  await page.getByLabel("Actual balance (EUR)").fill("1234.50");
  await page.getByLabel("Balance date", { exact: true }).fill("2026-10-13");
  await page
    .getByLabel("I confirm this is the account balance for the stated date.")
    .check();
  await page.getByRole("button", { name: "Save confirmed balance" }).click();
  await expect(page.locator(".balance-value")).toContainText("1,234.50");
  await expect(page.locator(".status-badge")).toHaveText("Confirmed by you");
  await expect(
    page.getByRole("heading", { name: "History coverage unverified" }),
  ).toBeVisible();
});
