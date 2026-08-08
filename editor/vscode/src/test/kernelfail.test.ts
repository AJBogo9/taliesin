// When a diagnostic means "the environment could not run this cell", and when to say so.
//
// This file is a DRIFT GATE as much as a unit test: the needles below are Rust format
// strings, written in `crates/server/src/build.rs`'s `cell_error_message` and
// `run_print.rs`'s `failure_line`. Reword one there and the doctor hint goes silent with
// every Rust gate still green, which is exactly what this catches.
import { test } from "node:test";
import assert from "node:assert";
import { kernelFailure, KernelPrompt, KERNEL_MESSAGES } from "../kernelfail";

test("the needles are the two the engine really writes", () => {
  // Both belong to the environment-failed family and nothing else: "did not run" is no
  // kernel at all, "did not complete" is one that died or was interrupted mid-cell. The
  // author's code raising says "raised an uncaught exception", which is a different fix.
  assert.deepStrictEqual(
    [...KERNEL_MESSAGES],
    ["code cell did not run", "code cell did not complete"]
  );
});

test("a kernel failure is recognised from the message", () => {
  // Until 2026-08-08 this keyed on a `TAL-KERNEL` code, which arrived as `{value, target}`
  // from the language client and as a bare string from a problem matcher, so two shapes had
  // to be understood, and a `d.code === "TAL-KERNEL"` check passed against a hand-made
  // fixture then never fired once in the editor. The catalogue is gone; the message is the
  // whole diagnostic, and there is one shape.
  assert.strictEqual(
    kernelFailure([{ message: "code cell did not run (no kernel was available)" }]),
    "code cell did not run (no kernel was available)"
  );
  assert.strictEqual(
    kernelFailure([{ message: "p.tmd: code cell did not complete (cell 2/4)" }]),
    "p.tmd: code cell did not complete (cell 2/4)"
  );
});

test("it returns the engine's own words, so the explanation is not a second copy", () => {
  // The reason the notification quotes the diagnostic instead of restating it: a sentence
  // written here is free to drift from the one the author already read in the gutter.
  const m = "code cell did not run: no Python kernel; install ipykernel";
  assert.strictEqual(kernelFailure([{ message: m }]), m);
});

test("another Taliesin diagnostic does not offer doctor", () => {
  // `doctor` audits the interpreter environment. Offering it for a broken cross-reference
  // would be advice that cannot help, on the busiest diagnostic in the tool.
  assert.strictEqual(kernelFailure([{ message: "broken cross-reference: @fig-x" }]), null);
  // Nor for the author's own code raising, which `doctor` cannot diagnose.
  assert.strictEqual(
    kernelFailure([{ message: "code cell raised an uncaught exception (cell 1/3)" }]),
    null
  );
  assert.strictEqual(kernelFailure([]), null);
});

test("the prompt fires once, then stays quiet for the rest of the session", () => {
  // Diagnostics are republished on every keystroke, so an unlatched watcher shows the same
  // notification on every edit until the kernel is fixed — which is the state it is trying
  // to help with.
  const fail = { message: "code cell did not run" };
  const prompt = new KernelPrompt();
  assert.strictEqual(prompt.offer([fail]), "code cell did not run");
  assert.strictEqual(prompt.offer([fail]), null);
  assert.strictEqual(prompt.offer([{ message: "code cell did not run twice" }]), null);
});

test("a session with no kernel failure never latches", () => {
  const prompt = new KernelPrompt();
  assert.strictEqual(prompt.offer([{ message: "broken cross-reference: @fig-x" }]), null);
  assert.strictEqual(prompt.offer([{ message: "code cell did not run" }]), "code cell did not run");
});
