#!/usr/bin/env python3
"""Drive `taliesin lsp` over real stdio and report what it answers, and how fast."""
import json, os, subprocess, sys, time, threading

BIN = sys.argv[1]
DOC = os.path.abspath(sys.argv[2])
ROOT = os.path.abspath(sys.argv[3])
text = open(DOC, encoding="utf-8").read()
uri = "file://" + DOC

p = subprocess.Popen([BIN, "lsp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL)

def send(obj):
    b = json.dumps(obj).encode()
    p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b)
    p.stdin.flush()

def read():
    n = None
    while True:
        line = p.stdout.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        if line.lower().startswith(b"content-length:"):
            n = int(line.split(b":")[1])
    return json.loads(p.stdout.read(n))

def rpc(i, method, params, timeout=10.0):
    t0 = time.perf_counter()
    send({"jsonrpc": "2.0", "id": i, "method": method, "params": params})
    while True:
        m = read()
        if m is None:
            return None, None
        if m.get("id") == i:
            return m, (time.perf_counter() - t0) * 1000

send({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
    "processId": os.getpid(), "rootUri": "file://" + ROOT,
    "capabilities": {"textDocument": {
        "publishDiagnostics": {}, "diagnostic": {"dynamicRegistration": True},
        "semanticTokens": {"requests": {"full": True}, "tokenTypes": [], "tokenModifiers": [],
                           "formats": ["relative"]},
        "references": {}, "selectionRange": {}, "codeLens": {},
    }, "workspace": {"diagnostics": {"refreshSupport": True}}}}})
init = read()
caps = init["result"]["capabilities"]
print("=== ADVERTISED CAPABILITIES ===")
for k in sorted(caps):
    print(f"  {k}")
send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
    "textDocument": {"uri": uri, "languageId": "tmd", "version": 1, "text": text}}})

lines = text.split("\n")
mid = len(lines) // 2
pos = {"line": mid, "character": 0}

print("\n=== REQUEST PROBE (does the server answer?) ===")
probes = [
    ("textDocument/hover", {"textDocument": {"uri": uri}, "position": pos}),
    ("textDocument/completion", {"textDocument": {"uri": uri}, "position": pos,
                                 "context": {"triggerKind": 1}}),
    ("textDocument/documentSymbol", {"textDocument": {"uri": uri}}),
    ("textDocument/foldingRange", {"textDocument": {"uri": uri}}),
    ("textDocument/inlayHint", {"textDocument": {"uri": uri},
                                "range": {"start": {"line": 0, "character": 0},
                                          "end": {"line": min(60, len(lines) - 1), "character": 0}}}),
    ("textDocument/documentLink", {"textDocument": {"uri": uri}}),
    ("textDocument/documentHighlight", {"textDocument": {"uri": uri}, "position": pos}),
    ("textDocument/formatting", {"textDocument": {"uri": uri},
                                 "options": {"tabSize": 2, "insertSpaces": True}}),
    ("workspace/symbol", {"query": "the"}),
    # --- the ones we suspect are missing ---
    ("textDocument/references", {"textDocument": {"uri": uri}, "position": pos,
                                 "context": {"includeDeclaration": True}}),
    ("textDocument/diagnostic", {"textDocument": {"uri": uri}}),
    ("workspace/diagnostic", {"previousResultIds": []}),
    ("textDocument/semanticTokens/full", {"textDocument": {"uri": uri}}),
    ("textDocument/codeLens", {"textDocument": {"uri": uri}}),
    ("textDocument/selectionRange", {"textDocument": {"uri": uri}, "positions": [pos]}),
    ("textDocument/signatureHelp", {"textDocument": {"uri": uri}, "position": pos}),
    ("textDocument/typeDefinition", {"textDocument": {"uri": uri}, "position": pos}),
    ("textDocument/declaration", {"textDocument": {"uri": uri}, "position": pos}),
    ("textDocument/linkedEditingRange", {"textDocument": {"uri": uri}, "position": pos}),
    ("textDocument/prepareCallHierarchy", {"textDocument": {"uri": uri}, "position": pos}),
    ("textDocument/inlineCompletion", {"textDocument": {"uri": uri}, "position": pos,
                                       "context": {"triggerKind": 1}}),
    ("textDocument/onTypeFormatting", {"textDocument": {"uri": uri}, "position": pos, "ch": "\n",
                                       "options": {"tabSize": 2, "insertSpaces": True}}),
    ("textDocument/documentColor", {"textDocument": {"uri": uri}}),
]
for i, (method, params) in enumerate(probes, start=100):
    m, ms = rpc(i, method, params)
    if m is None:
        print(f"  {method:45s} SERVER DIED")
        break
    if "error" in m:
        print(f"  {method:45s} error {m['error'].get('code')} {m['error'].get('message','')[:50]}")
    else:
        r = m.get("result")
        n = len(r) if isinstance(r, list) else ("null" if r is None else "obj")
        print(f"  {method:45s} ok  ({n})  {ms:.1f} ms")

# This client declares `textDocument.diagnostic`, so the server chooses the LSP 3.17 PULL
# transport and deliberately sends NO `publishDiagnostics` — a client that supports pull keeps
# those results in a collection of its own, so a server doing both would double every finding.
# Waiting for a publish here therefore hangs forever, which is exactly what the first run of
# this probe after that landed did. The signal a pull client is owed is
# `workspace/diagnostic/refresh`, so that is what is timed.
WANTED = ("workspace/diagnostic/refresh" if "diagnosticProvider" in caps
          else "textDocument/publishDiagnostics")
print(f"\n=== KEYSTROKE LATENCY (didChange -> {WANTED}) ===")
lat = []
for k in range(6):
    t0 = time.perf_counter()
    send({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
        "textDocument": {"uri": uri, "version": 2 + k},
        "contentChanges": [{"text": text + "\n\nedit %d\n" % k}]}})
    while True:
        m = read()
        if m is None:
            break
        if m.get("method") == WANTED:
            lat.append((time.perf_counter() - t0) * 1000)
            # A server request needs a reply, or the server's own pending table never drains.
            if "id" in m:
                send({"jsonrpc": "2.0", "id": m["id"], "result": None})
            break
lat.sort()
if lat:
    print(f"  n={len(lat)}  median {lat[len(lat)//2]:.1f} ms  min {lat[0]:.1f}  max {lat[-1]:.1f}"
          f"   (includes the 120 ms debounce)")
else:
    print("  no signal — the server neither published nor asked for a refresh")

send({"jsonrpc": "2.0", "id": 999, "method": "shutdown", "params": None})
read()
send({"jsonrpc": "2.0", "method": "exit", "params": None})
p.wait(timeout=5)
