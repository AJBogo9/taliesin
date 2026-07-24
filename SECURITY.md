# Security Policy

Taliesin is a local, single-author development tool: it renders `.tmd` files to
HTML and serves a live preview on your own machine. It is not a hosted service.
The notes below describe its trust model so you can tell an expected behavior
from an actual vulnerability before reporting.

## Supported versions

Taliesin is pre-1.0 and ships from a single line of development. Only the latest
released version (currently the `0.2.x` series) receives security fixes. There is
no back-porting to older tags.

| Version | Supported          |
| ------- | ------------------ |
| latest `0.2.x` | yes         |
| anything older | no          |

## Reporting a vulnerability

Please report suspected vulnerabilities **privately**, not as a public issue:

1. Preferred: open a private advisory through GitHub's **Report a vulnerability**
   button on the repository's *Security* tab
   (<https://github.com/AJBogo9/taliesin/security/advisories/new>).
2. Fallback: email `andreas.bogossian9@gmail.com` with `[taliesin security]` in
   the subject.

A short proof-of-concept or the exact reproduction steps helps a lot. Since this
is a one-maintainer project, expect an initial acknowledgement within about a
week; please allow time for a fix before any public disclosure.

## Trust model (what is *not* a vulnerability)

Taliesin runs on the author's machine and trusts the documents that author edits.
The following are by design:

- **Code cells execute.** `{python}`, `{r}`, and `{js}` cells run against a warm
  kernel or in the browser. Opening and previewing a `.tmd` document runs its
  code, exactly like a Jupyter notebook. Do not preview documents you would not
  run. `--no-exec` renders cells as source instead.
- **The preview binds to loopback by default.** Access is loopback-always; the
  websocket enforces an origin check. Passing `--host` exposes the server on your
  LAN behind a per-session token (`HttpOnly`, `SameSite=Lax`) printed at startup.
  The LAN transport is plain HTTP, so treat `--host` as "trusted local network
  only", never the public internet.
- **The preview is a read-only view.** It never writes back to your source;
  click-to-source only navigates the editor.
- **Symlinks inside your checkout are followed.** A `{{< include >}}`, `css:`,
  `bibliography:` or other resource path may resolve through a symlink to anywhere
  in the enclosing repository (the nearest ancestor holding `.git`), so sibling
  project directories can share one file. The document *text* is held to a narrower
  boundary: an absolute path or a `../` climb above the project root is refused, and
  so is a symlink whose target leaves the checkout. `build` applies the same rule when it
  mirrors assets into the output, so a link out of the checkout is never published. The
  `preview` server's asset endpoint is stricter still: it serves only what canonicalizes
  under the document's own directory, so a symlinked image that builds fine may 404 there.

Reports that fall inside this model (for example, "a cell can run arbitrary
code") are working as intended. Reports that let an *untrusted* document or a
*remote* page cross one of these boundaries (read files outside the project,
reach the server without the token, inject script into the rendered page) are in
scope and welcome.
