# claude-picker — landing page

The marketing site for [claude-picker](https://github.com/anshul-garg27/claude-picker).

Plain HTML, one stylesheet, one small script. No build step, no `node_modules`,
no framework. Edit, reload, ship.

## Structure

```
website/
├── index.html            main page
├── style.css             custom CSS (Catppuccin Mocha, self-hosted Geist)
├── script.js             picker-sim + cmd+k palette + scroll reveal + GH stats
├── vercel.json           headers + cache control for Vercel
├── netlify.toml          headers + cache control for Netlify
├── robots.txt            allow all + sitemap
├── sitemap.xml           single URL
├── CNAME                 empty — fill in for GitHub Pages custom domain
└── assets/
    ├── fonts/            self-hosted Geist + Geist Mono (.woff2, ~170KB)
    ├── favicon.svg       SVG favicon (modern browsers) — 4-dot mark
    ├── favicon.ico       multi-size .ico (16, 32, 48, 64)
    ├── logo.svg          wordmark + mark
    ├── mark.svg          4-dot mark (used in nav/footer)
    ├── og-image.png      1280x640 social preview
    ├── og-image.svg      source file for the OG image (legacy)
    ├── gifs/hero.gif     hero demo (used as no-JS fallback)
    └── mockups/          sessions, stats, before
```

## Interactive features

Everything works without JS (progressive enhancement). With JS, you also get:

- **Interactive picker** — hero centerpiece. Arrow keys, Enter to resume, type to fuzzy-filter, Escape to clear. Each session has a unique preview.
- **Command palette** — `⌘K` or `/` opens. Filters sections, opens external links.
- **Leader-key nav** — `g g` top, `g i` install, `g c` commands, `g f` features, `g h` GitHub.
- **Live GitHub stats** — stars, forks, issues, last-commit age. Cached 10 min in `sessionStorage`.
- **Scroll-reveal animations** — respects `prefers-reduced-motion`.
- **Animated star count-up** when the GitHub stats section scrolls into view.

Every image referenced by `index.html` lives inside `website/assets/`, so the
site is fully self-contained and drag-and-drop deployable.

## Local development

No install step. Any static server works:

```bash
python3 -m http.server 8000
# open http://localhost:8000
```

Or:

```bash
npx -y serve .
```

Or open `index.html` directly in a browser (some features like fonts and the
clipboard API want an http origin, so prefer the server).

## Deploy

### Vercel

Drag the `website/` directory onto [vercel.com/new](https://vercel.com/new),
or from the CLI:

```bash
vercel deploy --prod
```

`vercel.json` already sets security headers and long-cache on static assets.

### Netlify

Drag the `website/` directory onto [app.netlify.com/drop](https://app.netlify.com/drop),
or from the CLI:

```bash
netlify deploy --dir=website --prod
```

`netlify.toml` sets the same headers.

### GitHub Pages

Two options:

**Option A — repository Pages pointing to `/website`:**
In the repo's *Settings → Pages*, choose *Deploy from a branch*, select
`main` and folder `/website`. Done.

**Option B — `gh-pages` branch:**

```bash
git subtree push --prefix website origin gh-pages
```

### Custom domain

1. Point your DNS to the host:
   - Vercel / Netlify: add the CNAME they show you
   - GitHub Pages: add a `CNAME A` record pointing to `185.199.108–111.153`
2. Write your domain into `CNAME` (one line, no protocol):

```
claude-picker.dev
```

3. Update `<link rel="canonical">`, `og:url`, and `sitemap.xml` in
   `index.html` / `sitemap.xml` to match.

## Replacing assets

- **OG image** — if you regenerate it (e.g. from Gemini or Figma), save it as
  `assets/og-image.png` (1200x630, under 200 kB). The SVG source in
  `assets/og-image.svg` can be re-rendered with:

  ```bash
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
      --headless --disable-gpu --hide-scrollbars --window-size=1200,630 \
      --screenshot=assets/og-image.png \
      "file://$(pwd)/assets/og-image.svg"
  ```

- **Demo GIFs / mockups** — re-copy from `../assets/` if you regenerate them
  upstream:

  ```bash
  cp ../assets/gifs/hero.gif assets/gifs/
  cp ../assets/mockups/{sessions,stats,before}.png assets/mockups/
  ```

## Updating fonts

Fonts are self-hosted to eliminate the Google Fonts network hop. They live in
`assets/fonts/` as `.woff2` (Latin subsets, ~28 KB each). To refresh:

```bash
cd website/assets/fonts
curl -sS -o Geist-Regular.woff2  https://cdn.jsdelivr.net/npm/geist@1.4.2/dist/fonts/geist-sans/Geist-Regular.woff2
curl -sS -o Geist-Medium.woff2   https://cdn.jsdelivr.net/npm/geist@1.4.2/dist/fonts/geist-sans/Geist-Medium.woff2
curl -sS -o Geist-SemiBold.woff2 https://cdn.jsdelivr.net/npm/geist@1.4.2/dist/fonts/geist-sans/Geist-SemiBold.woff2
curl -sS -o Geist-Bold.woff2     https://cdn.jsdelivr.net/npm/geist@1.4.2/dist/fonts/geist-sans/Geist-Bold.woff2
curl -sS -o GeistMono-Regular.woff2 https://cdn.jsdelivr.net/npm/geist@1.4.2/dist/fonts/geist-mono/GeistMono-Regular.woff2
curl -sS -o GeistMono-Medium.woff2  https://cdn.jsdelivr.net/npm/geist@1.4.2/dist/fonts/geist-mono/GeistMono-Medium.woff2
```

## Performance

At launch: desktop Lighthouse 100/100/100 (a11y/best-practices/SEO). LCP ~56 ms.
CLS 0.00. Total initial download ~200 KB.

## What's deliberately missing

- No analytics. No cookies. No popups. No newsletter form.
- No React / Vue / Svelte / Astro / Tailwind build.
- No dark/light toggle — dark only, by design.
- No external CDN at runtime (fonts self-hosted). Only the GitHub API fetch
  goes off-site, and it degrades gracefully.
