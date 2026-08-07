#!/usr/bin/env python3
"""Does the LSP refresh an OPEN buffer's diagnostics when the file it depends on changes?

Builds a tiny 2-page site: page A links to a heading anchor on page B.
1. Break the link on disk (edit B *outside* the editor) -> does A's squiggle appear?
   (client sends workspace/didChangeWatchedFiles; server does not advertise or handle it)
2. Fix B *inside* the editor (didOpen + didChange on B) -> does A's squiggle clear?
"""
import json, os, shutil, subprocess, sys, tempfile, time

BIN = os.path.abspath(sys.argv[1])
root = tempfile.mkdtemp(prefix="tal-lsp-")
open(os.path.join(root, "_site.yml"), "w").write("title: probe\n")
A = os.path.join(root, "a.tmd")
B = os.path.join(root, "b.tmd")
open(A, "w").write("---\ntitle: A\n---\n\nSee [the section](b.html#target).\n")
open(B, "w").write("---\ntitle: B\n---\n\n## Target\n\nbody\n")

p = subprocess.Popen([BIN, "lsp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL)

def send(o):
    b = json.dumps(o).encode()
    p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b); p.stdin.flush()

def read(timeout=3.0):
    import select
    if not select.select([p.stdout], [], [], timeout)[0]:
        return None
    n = None
    while True:
        line = p.stdout.readline()
        if not line: return None
        if line in (b"\r\n", b"\n"): break
        if line.lower().startswith(b"content-length:"): n = int(line.split(b":")[1])
    return json.loads(p.stdout.read(n))

def drain_diags(secs=1.2):
    """Collect every publishDiagnostics that arrives in the next `secs`."""
    out = {}
    end = time.time() + secs
    while time.time() < end:
        m = read(timeout=max(0.05, end - time.time()))
        if m is None: continue
        if m.get("method") == "textDocument/publishDiagnostics":
            pr = m["params"]
            out[os.path.basename(pr["uri"])] = [d["message"] for d in pr["diagnostics"]]
    return out

send({"jsonrpc":"2.0","id":0,"method":"initialize","params":{
    "processId":os.getpid(),"rootUri":"file://"+root,
    "capabilities":{"workspace":{"didChangeWatchedFiles":{"dynamicRegistration":True}}}}})
init = read()
ws = init["result"]["capabilities"].get("workspace")
print("server 'workspace' capability block:", json.dumps(ws))
send({"jsonrpc":"2.0","method":"initialized","params":{}})

uriA = "file://" + A
send({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{
    "uri":uriA,"languageId":"tmd","version":1,"text":open(A).read()}}})
print("\n[1] A opened, link valid          ->", drain_diags())

# --- break B on disk, exactly as an external edit / git checkout would ---
open(B, "w").write("---\ntitle: B\n---\n\n## Renamed\n\nbody\n")
send({"jsonrpc":"2.0","method":"workspace/didChangeWatchedFiles","params":{
    "changes":[{"uri":"file://"+B,"type":2}]}})
print("[2] B's heading renamed on disk   ->", drain_diags(),
      "   <- A should now report a broken anchor")

# --- prove the diagnostic EXISTS by forcing a re-lint of A ---
send({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
    "textDocument":{"uri":uriA,"version":2},
    "contentChanges":[{"text":open(A).read()+"\n"}]}})
print("[3] after retyping in A           ->", drain_diags())

# --- does editing B *in the editor* refresh A? ---
uriB = "file://" + B
send({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{
    "uri":uriB,"languageId":"tmd","version":1,"text":open(B).read()}}})
drain_diags(0.4)
send({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
    "textDocument":{"uri":uriB,"version":2},
    "contentChanges":[{"text":"---\ntitle: B\n---\n\n## Target\n\nbody\n"}]}})
print("[4] B fixed in-editor (A untouched)->", drain_diags(),
      "   <- A's stale error should clear")

# --- what `check` says, as ground truth ---
open(B, "w").write("---\ntitle: B\n---\n\n## Renamed\n\nbody\n")
r = subprocess.run([BIN, "check", root, "--format", "json"], capture_output=True, text=True)
try:
    j = json.loads(r.stdout)
    ds = j.get("diagnostics", j if isinstance(j, list) else [])
    print("\n[ground truth] `taliesin check` on the broken tree:",
          [(d.get("code"), os.path.basename(str(d.get("file"))), d.get("message")) for d in ds])
except Exception:
    print("\n[ground truth] check stdout:", r.stdout[:400])

send({"jsonrpc":"2.0","id":99,"method":"shutdown","params":None}); read()
send({"jsonrpc":"2.0","method":"exit","params":None}); p.wait(timeout=5)
shutil.rmtree(root, ignore_errors=True)
