# claude-picker — landing page

The marketing site for [claude-picker](https://github.com/anshul-garg27/claude-picker).

Plain HTML, one stylesheet, one small script. No build step, no `node_modules`,
no framework. Edit, reload, ship.

## Structure

```
website/
├── index.html            main page
├── style.css             custom CSS (Catppuccin Mocha)
├── script.js             copy-button + smooth-scroll (~1.8 kB)
├── vercel.json           headers + cache control for Vercel
├── netlify.toml          headers + cache control for Netlify
├── robots.txt            allow all + sitemap
├── sitemap.xml           single URL
├── CNAME                 empty — fill in for GitHub Pages custom domain
└── assets/
    ├── favicon.svg       SVG favicon (modern browsers)
    ├── favicon.ico       multi-size .ico (16, 32, 48, 64)
    ├── logo.svg          wordmark + mark
    ├── mark.svg          glyph only (used in nav/footer)
    ├── og-image.png      1200x630 social preview
    ├── og-image.svg      source file for the OG image
    ├── gifs/hero.gif     hero demo (copied from ../assets/)
    └── mockups/          sessions, stats, before (copied from ../assets/)
```

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

## What's deliberately missing

- No analytics. No cookies. No popups. No newsletter form.
- No React / Vue / Svelte / Astro / Tailwind build.
- No dark/light toggle — dark only, by design.
