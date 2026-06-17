---
name: deploy
description: Run the blog's deploy ritual — regenerate llms-full.txt, render the site, and push _site to the cf-pages branch for Cloudflare Pages. Use when the user wants to publish, deploy, or ship the blog live.
---

# Deploy the blog

The site is a Quarto static build hosted on Cloudflare Pages (CNAME → andreasbogossian.com). Deployment is scripted in `publish.sh`, which renders the site and force-pushes `_site/` to the `cf-pages` branch.

## Preflight (do this first)

1. **Confirm intent.** Deploying is outward-facing and force-pushes a branch. Confirm the user wants to publish now unless they already said so explicitly.
2. **Check the working tree** with `git status`. If there are uncommitted source changes, surface them — the user likely wants those committed (with their approval) before the rendered output reflects them.
3. **Recommend a local check** if anything substantive changed: `quarto preview` to eyeball rendering before shipping.

## Deploy

Run the existing script from the repo root:

```bash
./publish.sh
```

It runs `generate_llms_full.py`, `quarto render`, then inits a throwaway git repo inside `_site/`, commits, and force-pushes to the `cf-pages` branch of the same origin remote. Cloudflare Pages builds from that branch.

## After

- Report the outcome faithfully: if render or push failed, show the error rather than claiming success.
- Cloudflare Pages takes a moment to build; the change is not live the instant the push returns.

## House rules

- Never commit source changes without explicit user approval (the deploy script's commit inside `_site/` is separate throwaway state, not your repo history).
