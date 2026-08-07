// When a diagnostic means "the environment could not run this cell", and when to say so.
//
// The shape assertions here are MEASURED, not assumed: a probe in the real Extension Host
// printed what `vscode.languages.getDiagnostics` actually hands back for a Taliesin
// diagnostic, and it is not a string. See the `code_description` note below.
import { test } from "node:test";
import assert from "node:assert";
import { kernelFailure, KernelPrompt, KERNEL_CODE } from "../kernelfail";

/** The wrapped shape the language client really delivers. */
function wrapped(code: string, message = "code cell did not run") {
  return {
    code: { value: code, target: { scheme: "https", path: "/DIAGNOSTICS.md" } },
    message,
  };
}

test("the kernel code is the one the diagnostic catalogue defines", () => {
  // `crates/core/src/diagnostics/codes.rs`: "code cell did not run" / "did not complete".
  assert.strictEqual(KERNEL_CODE, "TAL-KERNEL");
});

test("a kernel failure is recognised through the code_description wrapper", () => {
  // MEASURED in a real Extension Host: because `check.rs` wires `code_description` from
  // `docs_url`, vscode-languageclient delivers `code` as `{value, target}` — never as a
  // bare string. A `d.code === "TAL-KERNEL"` check passes against a hand-made fixture and
  // then never fires once in the editor, which is the whole failure this test exists for.
  assert.strictEqual(kernelFailure([wrapped("TAL-KERNEL")]), "code cell did not run");
});

test("a bare string code is recognised too", () => {
  // The other producer: a problem matcher writes `code` as a plain string. Both paths have
  // to work, because which one published a diagnostic is not something the watcher can see.
  assert.strictEqual(
    kernelFailure([{ code: "TAL-KERNEL", message: "code cell did not complete" }]),
    "code cell did not complete"
  );
});

test("it returns the engine's own words, so the explanation is not a second copy", () => {
  // The reason the notification quotes the diagnostic instead of restating it: the cause
  // and the fix live in the Rust catalogue, and a sentence written here would drift.
  assert.strictEqual(
    kernelFailure([wrapped("TAL-KERNEL", "no Python kernel: install ipykernel")]),
    "no Python kernel: install ipykernel"
  );
});

test("another Taliesin diagnostic does not offer doctor", () => {
  // `doctor` audits the interpreter environment. Offering it for a broken cross-reference
  // would be advice that cannot help, on the busiest diagnostic in the tool.
  assert.strictEqual(kernelFailure([wrapped("TAL-XREF-UNDEF", "broken cross-reference")]), null);
  assert.strictEqual(kernelFailure([]), null);
  assert.strictEqual(kernelFailure([{ message: "no code at all" }]), null);
});

test("a numeric code from some other extension is not mistaken for ours", () => {
  assert.strictEqual(kernelFailure([{ code: 2304, message: "cannot find name" }]), null);
});

test("the prompt fires once, then stays quiet for the rest of the session", () => {
  // Diagnostics are republished on every keystroke, so an unlatched watcher shows the same
  // notification on every edit until the kernel is fixed — which is the state it is trying
  // to help with.
  const prompt = new KernelPrompt();
  assert.strictEqual(prompt.offer([wrapped("TAL-KERNEL")]), "code cell did not run");
  assert.strictEqual(prompt.offer([wrapped("TAL-KERNEL")]), null);
  assert.strictEqual(prompt.offer([wrapped("TAL-KERNEL", "another one")]), null);
});

test("a session with no kernel failure never latches", () => {
  const prompt = new KernelPrompt();
  assert.strictEqual(prompt.offer([wrapped("TAL-XREF-UNDEF")]), null);
  assert.strictEqual(prompt.offer([wrapped("TAL-KERNEL")]), "code cell did not run");
});
