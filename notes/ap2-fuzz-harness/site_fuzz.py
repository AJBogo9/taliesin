#!/usr/bin/env python3
"""Throw garbage _site.yml + hostile pages at the real `taliesin build <dir>`."""
import os, subprocess, tempfile, shutil, sys

WT = "/home/bogo/Documents/personal/taliesin/.claude/worktrees/audit-ap2-fuzzing"
TALI = f"{WT}/target/debug/taliesin"

def classify(code):
    return {0:"OK",1:"exit1(handled)",101:"PANIC",137:"HANG",134:"SIGABRT",
            139:"SEGV",2:"exit2",-6:"SIGABRT",-11:"SEGV",-9:"SIGKILL/HANG"}.get(code, f"exit={code}")

def run_site(label, config, pages=None, timeout=25):
    d = tempfile.mkdtemp(); out = tempfile.mkdtemp()
    try:
        with open(f"{d}/_site.yml", "w") as f:
            f.write(config)
        pages = pages or {"index.tmd": "---\ntitle: Home\n---\n\n# Home\n\nbody\n",
                          "page2.tmd": "---\ntitle: Two\n---\n\n## Two\n\nmore\n"}
        for name, content in pages.items():
            with open(f"{d}/{name}", "w") as f:
                f.write(content)
        try:
            r = subprocess.run([TALI, "build", d, "--out", out],
                               stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=timeout)
            code = r.returncode; err = r.stderr.decode('utf-8','replace')
        except subprocess.TimeoutExpired:
            code = -9; err = ""
        cls = classify(code)
        panic = ""
        for ln in err.splitlines():
            if any(k in ln.lower() for k in ("panic","overflow","backtrace","unwrap","index out")):
                panic = ln.strip()[:80]; break
        flag = "  <<<" if cls in ("PANIC","SIGABRT","SEGV","HANG") else ""
        print(f"{label:38s} {cls:16s} {panic}{flag}")
        return cls
    finally:
        shutil.rmtree(d, ignore_errors=True); shutil.rmtree(out, ignore_errors=True)

L = "".join("  "*i + f"k{i}:\n" for i in range(500))
BL = ("a: &a [x,x,x,x,x,x,x,x,x]\nb: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]\n"
      "c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]\nd: [*c,*c,*c,*c,*c,*c,*c,*c,*c]\n")

print("=== garbage _site.yml through real `taliesin build <dir>` ===")
run_site("empty config", "")
run_site("scalar (not a map)", "just a string")
run_site("list (not a map)", "- a\n- b\n")
run_site("unparseable yaml", "title: : : :\n  bad\n")
run_site("wrong-typed title (list)", "title: [1,2,3]\n")
run_site("wrong-typed navbar (int)", "navbar: 42\n")
run_site("navbar wrong shape", "navbar:\n  left: not-a-list\n")
run_site("huge scalar title", "title: " + "x"*200000 + "\n")
run_site("deep nested yaml", L)
run_site("billion laughs", BL)
run_site("unknown field flood", "\n".join(f"f{i}: v{i}" for i in range(5000)))
run_site("nulls", "title: null\nnavbar: null\nfooter: null\n")
run_site("bad book config", "book:\n  chapters: not-a-list\n")
run_site("bad listing", "listing:\n  contents: 99\n")
run_site("mounts wrong", "mounts: [1,2,3]\n")
run_site("theme wrong", "theme: [dark, light, mauve]\n")

print("\n=== hostile PAGES in a valid site (per-page catch_unwind should hold) ===")
run_site("page = 90k blockquote", "title: T\n",
         {"index.tmd":"---\ntitle: H\n---\n\nhi\n", "bad.tmd": ">"*90000 + " x\n"}, timeout=30)
run_site("page = dup section ids", "title: T\n",
         {"index.tmd":"---\ntitle: H\n---\n\n## X {#sec-a}\n\n## Y {#sec-a}\n\n@sec-a\n"})
run_site("page = truncated frontmatter", "title: T\n",
         {"index.tmd":"---\ntitle: H\n---\n\nhi\n", "bad.tmd":"---\ntitle: ok\nauthor"})
run_site("filename with unicode", "title: T\n",
         {"index.tmd":"---\ntitle: H\n---\n\nhi\n", "café\U0001f600.tmd":"---\ntitle: U\n---\n\nu\n"})
