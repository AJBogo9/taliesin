// Regression test for the buildErrored latch fix.
//
// The latch logic lives inside the client.js IIFE and uses many DOM APIs.
// Rather than importing client.js directly, this test replicates the exact
// latch logic that was changed so we can verify the three sequences without
// a full DOM environment.
//
// The three sequences:
//   (a) error -> idle (same build): chip must stay Error
//   (b) error -> warming-kernel -> idle (new build): chip must reach "Up to date"
//   (c) success (no error) -> idle: chip must reach "Up to date"

"use strict";

let passed = 0;
let failed = 0;

function assert(label, condition, detail) {
  if (condition) {
    console.log("PASS  " + label);
    passed++;
  } else {
    console.log("FAIL  " + label + (detail ? "  (" + detail + ")" : ""));
    failed++;
  }
}

// ---- Minimal stub of the latch state machine --------------------------------
// Mirrors the logic in client.js updateProgress exactly as patched.
// Returns a function `updateProgress(msg)` and a getter `getState()`.

function makeMachine() {
  var buildStartMs = null;
  var warmStartMs = null;
  var buildErrored = false;
  var dataState = "idle"; // tracks data-state attribute value

  function updateProgress(msg) {
    if (msg.phase === "idle") {
      if (buildErrored) return; // latch: don't clobber error chip
      buildStartMs = null;
      dataState = "idle";
      return;
    }

    // FIX: clear latch on a genuinely new build phase (warming-kernel or executing),
    // independent of buildStartMs.
    var isNewBuild = msg.phase === "warming-kernel" || msg.phase === "executing";
    if (isNewBuild) {
      buildErrored = false;
      if (buildStartMs === null) buildStartMs = Date.now();
    }

    if (msg.phase === "error") {
      buildErrored = true;
      dataState = "error";
      return;
    }
    if (msg.phase === "warming-kernel") {
      dataState = "warming";
      return;
    }
    // executing
    dataState = "busy";
  }

  return {
    updateProgress: updateProgress,
    getState: function () { return dataState; },
    isErrored: function () { return buildErrored; },
  };
}

// ---- Sequence (a): error -> idle  ------------------------------------------
// An `idle` that arrives after an error for the SAME build must NOT overwrite
// the error chip. The latch must hold.
(function () {
  var m = makeMachine();
  m.updateProgress({ phase: "executing", ran: 1, total: 2 });
  m.updateProgress({ phase: "error" });
  assert("(a) after error, data-state is error", m.getState() === "error");
  m.updateProgress({ phase: "idle" });
  assert("(a) idle after same-build error: chip stays Error",
    m.getState() === "error",
    "got: " + m.getState());
})();

// ---- Sequence (b): error -> warming-kernel -> idle -------------------------
// A NEW build starts (warming-kernel), which clears the latch, so the
// subsequent idle can flip the chip to "Up to date".
(function () {
  var m = makeMachine();
  // first build: fails
  m.updateProgress({ phase: "executing", ran: 0, total: 1 });
  m.updateProgress({ phase: "error" });
  assert("(b) after first build error, state is error", m.getState() === "error");
  assert("(b) buildErrored latch is set", m.isErrored() === true);

  // user fixes the file; server starts a new build
  m.updateProgress({ phase: "warming-kernel", lang: "python" });
  assert("(b) after warming-kernel, latch is cleared", m.isErrored() === false,
    "buildErrored=" + m.isErrored());
  assert("(b) after warming-kernel, state is warming", m.getState() === "warming");

  m.updateProgress({ phase: "executing", ran: 1, total: 1 });
  assert("(b) after executing, state is busy", m.getState() === "busy");

  m.updateProgress({ phase: "idle" });
  assert("(b) after idle following new build, state is idle (Up to date)",
    m.getState() === "idle",
    "got: " + m.getState());
})();

// ---- Sequence (b2): error -> executing -> idle  (no warming phase) ----------
// Same as (b) but the new build jumps straight to executing (no kernel warmup).
(function () {
  var m = makeMachine();
  m.updateProgress({ phase: "executing", ran: 0, total: 1 });
  m.updateProgress({ phase: "error" });
  assert("(b2) latch set after error", m.isErrored() === true);

  m.updateProgress({ phase: "executing", ran: 1, total: 1 });
  assert("(b2) latch cleared by new executing", m.isErrored() === false,
    "buildErrored=" + m.isErrored());

  m.updateProgress({ phase: "idle" });
  assert("(b2) idle after new build succeeds -> Up to date",
    m.getState() === "idle",
    "got: " + m.getState());
})();

// ---- Sequence (c): success -> idle -----------------------------------------
// A normal build with no errors should result in "Up to date" after idle.
(function () {
  var m = makeMachine();
  m.updateProgress({ phase: "warming-kernel", lang: "python" });
  m.updateProgress({ phase: "executing", ran: 1, total: 2 });
  m.updateProgress({ phase: "executing", ran: 2, total: 2 });
  m.updateProgress({ phase: "idle" });
  assert("(c) clean build -> idle -> Up to date",
    m.getState() === "idle",
    "got: " + m.getState());
  assert("(c) buildErrored remains false after clean build",
    m.isErrored() === false);
})();

// ---- Summary ---------------------------------------------------------------
console.log("");
console.log("Results: " + passed + " passed, " + failed + " failed");
process.exit(failed > 0 ? 1 : 0);
