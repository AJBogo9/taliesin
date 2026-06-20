# Visual & UX audit (2026-06-19)

In-browser audit (chrome-devtools) of all six public-facing surfaces, captured at
desktop + mobile, light + dark, with console + theme checks. Lens: this is about
*showing the project* and winning over **Quarto/Jupyter switchers**, not selling.

## Summary

The four format demos and the docs are in genuinely strong shape; the wow is real
and unfaked. Issues cluster on the **marketing site's hero pages**, which the
planned demo-machine rebuild replaces anyway. **Every console was clean** across all
six surfaces.

| Surface | Verdict | Notes |
|---|---|---|
| Slide deck (`corpus/liquid-glass-slides`) | Top-tier wow | Frosted-glass title slide + per-bullet glass panels; a third-party reveal theme renders on qmd-fast's own engine. Best single demo asset. |
| Blog post (`corpus/posts/em-algorithm`) | Strong | 90 KaTeX spans, executed Python with dark-mode-aware matplotlib, collapsible code folds, callouts. |
| Multi-page website (`corpus/tech-blog`) | Strong | Navbar, `about:` header, listing cards w/ thumbnails + tags, RSS, footer. Mobile wraps correctly. |
| Docs book (`docs/`) | Strong | Numbered Mermaid figures, component tables, section numbering, sub-TOC. Internals reads as a credibility asset. |
| Multi-chapter book (`corpus/demo-book`) | Solid | Parts, numbering, prev/next, sidebar. Same shell as docs (consistent engine). |
| Marketing site (`site/`) | Needs the planned rebuild | Text-led not demo-led; mobile overflow; theme/video desync; passive top CTA. |

## Bugs (fix regardless of redesign)

1. **[High] Mobile prose overflow** on the marketing hero pages (`page-layout: full`
   + `hero:`). The intro paragraph clips off the right edge at 390px. Isolated to
   those pages: the shared site chrome wraps fine (verified on tech-blog). Contained
   CSS fix in the full-layout body container.
2. **[Med] Theme / video desync.** Site chrome uses a manual toggle (defaults dark,
   ignores OS `prefers-color-scheme`); the `{{< video >}}` light/dark swap follows
   the OS media query. OS-light => light video inside a dark page. Drive the video
   variant off the site toggle, not the media query.
3. **[Low] Mermaid diagrams** run a little low-contrast (grey-on-dark) in the
   internals book.
4. **[Low] Em dashes** throughout the marketing copy (against the author's writing
   rule; also worth tightening for the leaner demo voice).

## Strategic direction (decided this session)

**Two separate docs books + a demo-machine website.**

- **User Guide** = `docs/using/` + `docs/reference/` (how to *use* the tool; the
  adoption funnel for switchers).
- **Internals** = `docs/internals/` (how it's *built*; a public credibility piece,
  written as explanation).
- **Website** = a demo machine: lead with motion above the fold, one crisp value
  line on top, a vs-Quarto table (reuse the one already in the docs index), a real
  install/quickstart on-ramp into the User Guide. Cap embedded slides at one hero
  deck per page.

**The two-book split is cheap:** only 2 cross-links to fix (both
`internals/ -> using/`, zero the other way), no shared `@sec-` cross-refs, numbering
just restarts per book.
