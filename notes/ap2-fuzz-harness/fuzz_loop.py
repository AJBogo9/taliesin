#!/usr/bin/env python3
"""AP2 generative mutation fuzzer over render_document.

Feeds each mutated input to the fuzz_render harness in a subprocess with a
timeout. Any non-zero exit (panic 101, abort 134, segv 139, hang 124/137) is a
finding: the input is saved to crashes/ for repro. Deterministic seed so the
run is reproducible (no wall-clock randomness).
"""
import os, sys, glob, random, subprocess, time

WT = "/home/bogo/Documents/personal/taliesin/.claude/worktrees/audit-ap2-fuzzing"
BIN = f"{WT}/target/debug/examples/fuzz_render"
CORPUS = glob.glob(f"{WT}/corpus/**/*.tmd", recursive=True) + glob.glob(f"{WT}/docs/**/*.tmd", recursive=True)
ITERS = int(sys.argv[1]) if len(sys.argv) > 1 else 3000
TIMEOUT = float(sys.argv[2]) if len(sys.argv) > 2 else 6.0
CRASHDIR = sys.argv[3] if len(sys.argv) > 3 else "crashes"
os.makedirs(CRASHDIR, exist_ok=True)
random.seed(0xA2F022)  # deterministic

seeds = []
for p in CORPUS:
    try:
        with open(p, "rb") as f:
            seeds.append(f.read())
    except OSError:
        pass

# "scary token" alphabet: structural markers most likely to reach edge code.
TOKENS = [
    b":::", b":::: {.callout-note}", b"::: {.columns}", b"::: {.column width=50%}",
    b"::: {.magic-move}", b"::: {.step lines=1-3}", b"::: {.notes}", b"::: {.panel-tabset}",
    b"```", b"```{python}", b"```{r}", b"```{js}", b"```{=html}", b"~~~", b"````",
    b"$", b"$$", b"\\frac{", b"}", b"\\begin{matrix}", b"\\def\\x{\\x}",
    b"#| echo: false", b"#| lines:", b"#| label: fig-x", b"//| ", b"%%| ",
    b"[", b"]", b"(", b")", b"![", b"[^1]", b"[^1]:", b"[@key]", b"@fig-x", b"@sec-y",
    b"{{< include x.tmd >}}", b"{{< embed d.tmd >}}", b"{{< video v.mp4 >}}", b"{{< ",
    b"| a | b |", b"| - | - |", b"|", b"---", b"# ", b"###### ", b"> ", b"- ", b"1. ",
    b"<div>", b"</div>", b"<!--", b"-->", b"\t", b"\r\n", b"\n\n",
    b"caf\xc3\xa9", b"\xe4\xb8\xad\xe6\x96\x87", b"\xf0\x9f\x98\x80", b"\xcc\x81",
    b"\xe2\x80\xae", b"\x00", b"title:", b"format: revealjs", b"prose-lint:",
]

def mut_soup():
    n = random.randint(3, 400)
    return b"".join(random.choice(TOKENS) for _ in range(n))

def mut_bytes():
    return bytes(random.randint(0, 255) for _ in range(random.randint(1, 2000)))

def mut_splice():
    if not seeds:
        return mut_soup()
    s = bytearray(random.choice(seeds))
    for _ in range(random.randint(1, 30)):
        op = random.random()
        pos = random.randint(0, max(0, len(s)))
        if op < 0.4:  # insert a scary token
            s[pos:pos] = random.choice(TOKENS)
        elif op < 0.6:  # delete a chunk
            end = min(len(s), pos + random.randint(1, 200))
            del s[pos:end]
        elif op < 0.75:  # duplicate a chunk (grow structure)
            end = min(len(s), pos + random.randint(1, 500))
            s[pos:pos] = s[pos:end]
        elif op < 0.9:  # truncate
            s = s[:pos]
        else:  # flip bytes
            for _ in range(random.randint(1, 20)):
                if s:
                    i = random.randint(0, len(s) - 1)
                    s[i] = random.randint(0, 255)
    return bytes(s)

def mut_repeat():
    tok = random.choice(TOKENS)
    return tok * random.randint(1000, 60000)

STRATS = [mut_soup, mut_bytes, mut_splice, mut_splice, mut_repeat]

crashes = 0
slow = []
t0 = time.time()
for i in range(ITERS):
    data = random.choice(STRATS)()
    try:
        st = time.time()
        r = subprocess.run([BIN, os.environ.get("MODE","doc")], input=data, stdout=subprocess.DEVNULL,
                           stderr=subprocess.PIPE, timeout=TIMEOUT)
        dt = time.time() - st
        code = r.returncode
        if code != 0:
            crashes += 1
            fn = f"{CRASHDIR}/crash_{i:05d}_exit{code}.tmd"
            with open(fn, "wb") as f:
                f.write(data)
            print(f"[{i}] CRASH exit={code} ({len(data)}B) -> {fn}")
            print("   " + (r.stderr.decode('utf-8', 'replace').strip().splitlines() or ["<no stderr>"])[0][:100])
        elif dt > TIMEOUT * 0.6:
            slow.append((dt, len(data), i))
    except subprocess.TimeoutExpired:
        crashes += 1
        fn = f"{CRASHDIR}/hang_{i:05d}.tmd"
        with open(fn, "wb") as f:
            f.write(data)
        print(f"[{i}] HANG >{TIMEOUT}s ({len(data)}B) -> {fn}")
    if i % 500 == 0 and i:
        print(f"... {i}/{ITERS}  crashes={crashes}  elapsed={time.time()-t0:.0f}s")

print(f"\nDONE {ITERS} iters in {time.time()-t0:.0f}s  crashes/hangs={crashes}")
if slow:
    slow.sort(reverse=True)
    print("slowest (near-timeout) cases:")
    for dt, ln, i in slow[:8]:
        print(f"   iter {i}: {dt:.2f}s  {ln}B")
