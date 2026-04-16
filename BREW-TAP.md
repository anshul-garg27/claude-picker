# Setting up the Homebrew tap

`cargo-dist` pushes the generated `claude-picker.rb` formula to
`anshul-garg27/homebrew-tap` on every tagged release. This file documents the
**one-time setup** the repo owner runs before the first release goes out.

---

## 1. Create the tap repository

The tap must be a public GitHub repo named **exactly** `homebrew-tap` under
your user/org (here: `anshul-garg27/homebrew-tap`). Homebrew derives the tap
name from the repo name, so this is not negotiable.

```bash
# Needs the GitHub CLI: https://cli.github.com/
gh repo create anshul-garg27/homebrew-tap \
  --public \
  --description "Homebrew tap for Anshul Garg's open-source tools"

git clone https://github.com/anshul-garg27/homebrew-tap.git /tmp/tap
cd /tmp/tap
mkdir -p Formula
printf '# Homebrew tap\n\n`brew tap anshul-garg27/tap`\n' > README.md
git add .
git commit -m "init tap"
git push
```

The `Formula/` directory is where `cargo-dist` will drop `claude-picker.rb`.

---

## 2. Create the GitHub secret

`cargo-dist` needs a token with `repo` scope to push commits to the tap on
your behalf. **Add it to the `claude-picker` repo, not the tap repo.**

1. Visit <https://github.com/settings/tokens> and generate a classic PAT
   (or a fine-grained token with write access to `anshul-garg27/homebrew-tap`).
   Select the **`repo`** scope.
2. Copy the token value.
3. In the `claude-picker` repo, go to **Settings → Secrets and variables →
   Actions → New repository secret**.
4. Name: `HOMEBREW_TAP_TOKEN`. Value: paste the PAT.

You can also do this from the CLI:

```bash
gh secret set HOMEBREW_TAP_TOKEN \
  --repo anshul-garg27/claude-picker \
  --body "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
```

---

## 3. Cut the first release

Any tag matching `v*.*.*` triggers `.github/workflows/release.yml`:

```bash
cd /path/to/claude-picker
git tag v0.2.0
git push origin v0.2.0
```

The workflow runs `cargo test` + `cargo clippy -- -D warnings`, builds
prebuilt tarballs for all five targets, uploads them to GitHub Releases, and
then the `publish-homebrew-formula` job commits the generated `claude-picker.rb`
to `anshul-garg27/homebrew-tap/Formula/`.

---

## 4. Verify

Once the workflow finishes:

```bash
brew tap anshul-garg27/tap
brew install claude-picker
claude-picker --help
```

`brew tap` infers the URL `https://github.com/anshul-garg27/homebrew-tap`
from the short name. From the user's perspective, there is one command —
`brew install anshul-garg27/tap/claude-picker` — and they're done.

---

## 5. Cutting subsequent releases

Bump the version in `Cargo.toml`, then tag:

```bash
# Edit Cargo.toml: version = "0.2.1"
git commit -am "release v0.2.1"
git tag v0.2.1
git push origin main v0.2.1
```

`cargo-dist` will handle the rest. Users upgrade with `brew upgrade claude-picker`
or by re-running the `curl | sh` installer.

---

## Troubleshooting

- **`publish-homebrew-formula` fails with `Permission denied`**: the PAT is
  missing `repo` scope, or the token has expired, or the secret name is
  misspelled. It must be exactly `HOMEBREW_TAP_TOKEN`.
- **`brew tap` can't find the repo**: the tap repo name must be
  `homebrew-tap`, not `claude-picker-tap` or anything else. Homebrew hardcodes
  this prefix.
- **Formula published but `brew install` 404s on the tarball**: the GitHub
  Release artifacts probably didn't finish uploading before the formula was
  committed. Re-run the failed `publish-homebrew-formula` job from the Actions
  UI — it's idempotent.
