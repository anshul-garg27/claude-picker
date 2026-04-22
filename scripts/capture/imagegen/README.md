# imagegen — hero banner prompt

The banner at [`../../../assets/hero-banner.jpg`](../../assets/hero-banner.jpg) was
generated from `prompt_full_banner.txt` against an image model that can render fine
monospace text (the shipped file was rendered through the ChatGPT web portal; the same
prompt also works against Azure OpenAI `gpt-image-2` at `quality=high`, though the
portal's result was the crispest).

## The prompt

[`prompt_full_banner.txt`](prompt_full_banner.txt) spells out every visible text string
exactly, plus strict rules that forbid text invention, Latin-only alphabet, and a
fixed Kanagawa palette. Because modern image models can now render short monospace
strings reliably when told to, the whole banner — wordmark, terminal content, 10 group
tabs, 6 project rows, and the hotkey bar — lives inside one prompt.

## Regenerating

```bash
# Via Azure OpenAI:
export AZURE_API_KEY=...
export AZURE_ENDPOINT=https://<your-resource>.openai.azure.com

curl -sS --fail \
  -H "Content-Type: application/json" \
  -H "api-key: $AZURE_API_KEY" \
  -d "$(jq -n --arg p "$(cat prompt_full_banner.txt)" \
    '{prompt:$p, n:1, size:"1536x1024", quality:"high"}')" \
  "$AZURE_ENDPOINT/openai/deployments/gpt-image-2/images/generations?api-version=2025-04-01-preview" \
  | jq -r '.data[0].b64_json' | base64 --decode > banner.png
```

High quality takes ~150–200s per image. The portal route can produce a different
(often sharper) result — iterate on whichever renderer gives the cleanest text.

## Post-processing

The raw output comes back at roughly 2 MB PNG. For the README we want <300 KB:

```bash
magick banner.png -strip -interlace Plane -quality 92 -sampling-factor 4:2:0 hero-banner.jpg
```

## Why the prompt is so verbose

`gpt-image-2` and its peers hallucinate text when left to their own devices — the
hero art retired in this release (`hero-marketing.png` with fake `$1,357.80` cost
rows and random kanji) is the cautionary tale. The fix is to **declare every glyph
explicitly**, pin the alphabet to Latin, spell out each separator (`·`, `•`, `▸`,
`▋`), and leave the model zero room to improvise. That one-page prompt is the whole
moat between a banner that looks like marketing fluff and one that reads like a
literal screenshot.
