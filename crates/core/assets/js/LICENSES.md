# Licenses for the vendored JavaScript in this directory

MIT and ISC both require that the **permission notice**, not merely the copyright
line, travels with every copy of the software. The minified bundles here carry only a
one-line copyright header (and `mermaid.min.js` carries none at all), so the full texts
live in this file, which ships beside them.

This covers the redistributed third-party bundles only. Taliesin's own scripts in this
directory (`mermaid.js`, `tali-js.js`, `tabset.js`, `walkthrough.js`,
`scrolly.js`, `glsl.js`, `numerics.js`, and the `code-enhance/` fragments) — and the
stylesheets in `../css/` — are covered by the project's own `LICENSE` at the repository
root.

**In a document you build, those own scripts and stylesheets carry the
[Taliesin Output Exception](../../../../LICENSE-OUTPUT-EXCEPTION.md)**, which lets you
publish that output under any terms with nothing to attribute. The AGPL still governs
them *as source in this repository*. That is why none of these files carries a licence
header: every byte here is copied verbatim into every page Taliesin builds, so a header
would add ~1 KB to each page to assert a licence the exception exists to disclaim. The
notice belongs here, where it costs a reader nothing.

---

## d3 v7.9.0 — `d3.min.js`

ISC License

Copyright 2010-2023 Mike Bostock

Permission to use, copy, modify, and/or distribute this software for any purpose
with or without fee is hereby granted, provided that the above copyright notice
and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.

---

## Observable Plot v0.6.16 — `plot.umd.min.js`

ISC License

Copyright 2020-2023 Observable, Inc.

Permission to use, copy, modify, and/or distribute this software for any purpose
with or without fee is hereby granted, provided that the above copyright notice
and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.

---

## Mermaid v11.16.0 — `mermaid.min.js`

MIT License

Copyright (c) 2014 - 2024 Knut Sveidqvist

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

### Mermaid's own bundled dependencies

`mermaid.min.js` is an esbuild bundle that inlines its dependency tree, so the
following are redistributed inside that one file. Each is under its own license;
consult the upstream project for the authoritative text.

| Component | License | Upstream |
| --- | --- | --- |
| DOMPurify | Apache-2.0 OR MPL-2.0 | <https://github.com/cure53/DOMPurify> |
| cytoscape | MIT | <https://github.com/cytoscape/cytoscape.js> |
| dagre / dagre-d3-es | MIT | <https://github.com/cytoscape/dagre> |
| marked | MIT | <https://github.com/markedjs/marked> |
| KaTeX | MIT | <https://github.com/KaTeX/KaTeX> |
| lodash-es | MIT | <https://github.com/lodash/lodash> |
| d3 | ISC | <https://github.com/d3/d3> |

Upstream ships a consolidated notices file; when bumping the vendored bundle, refresh
this table from mermaid's own `THIRD-PARTY-NOTICES` for that release.

