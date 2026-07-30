// What the status bar says, as a pure function of what the extension knows.
//
// The whole surface is two facts the extension host already has: whether a preview is running
// for this project, and how many problems the last `check` found. Live kernel and cache state
// is deliberately absent, and that is a design decision rather than an omission: the webview
// relay carries exactly four message types on purpose, and none of them reports kernel
// liveness. Widening the relay is a protocol decision to make on its own merits.
import { test } from "node:test";
import assert from "node:assert";
import { statusText, statusTooltip } from "../statustext";

test("a running preview shows its port", () => {
  assert.strictEqual(statusText({ previewPort: 4388, problems: 0 }), "$(book) Taliesin :4388");
});

test("problems are shown when there are any", () => {
  assert.strictEqual(
    statusText({ previewPort: 4388, problems: 3 }),
    "$(book) Taliesin :4388 · 3 problems"
  );
});

test("one problem is not pluralised", () => {
  assert.strictEqual(statusText({ previewPort: null, problems: 1 }), "$(book) Taliesin · 1 problem");
});

test("an unknown problem count is omitted rather than shown as zero", () => {
  // `null` means check has not run yet. Rendering that as "0 problems" claims a clean project
  // the extension has not verified.
  assert.strictEqual(statusText({ previewPort: null, problems: null }), "$(book) Taliesin");
});

test("a clean project shows no problem count either", () => {
  // Zero is worth saying only next to something that could be non-zero; on its own it is
  // noise in a status bar the author reads at a glance.
  assert.strictEqual(statusText({ previewPort: null, problems: 0 }), "$(book) Taliesin");
});

test("the tooltip distinguishes no preview from a running one", () => {
  assert.match(statusTooltip({ previewPort: null, problems: null }), /open a preview/i);
  assert.match(statusTooltip({ previewPort: 4388, problems: null }), /4388/);
});

test("the tooltip says check has not run, rather than implying the project is clean", () => {
  assert.match(statusTooltip({ previewPort: null, problems: null }), /not run/i);
  assert.match(statusTooltip({ previewPort: null, problems: 0 }), /no problems/i);
});
