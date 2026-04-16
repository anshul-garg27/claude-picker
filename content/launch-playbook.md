# claude-picker Launch Playbook

Based on deep research of 50+ sources, competitor analysis, and successful open-source launch patterns.

---

## Positioning

> **claude-picker**: Find any Claude Code conversation by content, see what it cost you, and resume it — in one keystroke. The Unix way.

**Unique angle vs competitors:**
- claude-history (Rust) — requires Rust toolchain. We're bash/fzf, zero compile.
- Claude Squad (Go) — orchestrator, not a picker. Different category.
- ccmanager — multi-agent focus. We're Claude-first, lightweight.
- **No other picker shows per-session cost/tokens.** This is our moat.

---

## Pre-Launch (1-2 weeks before)

- [ ] Join and participate in r/ClaudeAI, r/ClaudeCode, r/commandline (build karma)
- [ ] Comment helpfully on Claude Code session management threads
- [ ] Seed 100-200 GitHub stars from personal network (credibility floor)
- [ ] Record demo GIF (screen recording + ffmpeg)
- [ ] Generate all images via Gemini AI Pro
- [ ] Finalize README with GIF above the fold
- [ ] Pre-write ALL posts (HN, Reddit x3, Dev.to, Twitter, LinkedIn)
- [ ] Prepare newsletter pitch emails

---

## Launch Day (Tuesday/Wednesday/Thursday)

| Time (ET) | Platform | Action |
|-----------|----------|--------|
| 7:00 AM | **Hacker News** | Show HN post + founder comment within 5 min |
| 8:00 AM | **Reddit r/ClaudeCode** | Personal story post with demo GIF |
| 8:15 AM | **Reddit r/ClaudeAI** | Cross-post |
| 8:30 AM | **Reddit r/commandline** | Technical/fzf angle |
| 9:00 AM | **Twitter/X** | Thread (6 tweets) with terminal video |
| 9:00 AM | **Dev.to** | Full article (primary blog) |
| 10:00 AM | **LinkedIn** | Carousel post |
| 10:00 AM | **Email TLDR** | submissions@tldr.tech — 2-sentence pitch |
| 10:00 AM | **Discord** | Charm.sh, AI Engineering, Claude Code communities |
| 12:00 PM | **Hashnode** | Cross-post from Dev.to |

---

## Day 2

- [ ] Respond to EVERY comment on HN, Reddit, Twitter
- [ ] Submit to awesome lists:
  - [ ] hesreallyhim/awesome-claude-code (use issue template)
  - [ ] rohitg00/awesome-claude-code-toolkit
  - [ ] travisvn/awesome-claude-skills
  - [ ] rosaboyle/awesome-cc-oss
  - [ ] awesome-cli-apps
  - [ ] awesome-shell
- [ ] Submit to Console.dev and Changelog
- [ ] Product Hunt launch (12:01 AM PT)

---

## Day 3-7

- [ ] Continue responding to all comments
- [ ] "Building in public" follow-up post with star growth screenshot
- [ ] Contact newsletter curators with social proof

---

## Content Targets

| Platform | Format | Primary article location |
|----------|--------|------------------------|
| Dev.to | Full article | **PRIMARY** (auto-surfaces on daily.dev) |
| Medium | Cross-post to Level Up Coding or Bootcamp publication | SECONDARY |
| Hashnode | Cross-post with canonical URL to Dev.to | SECONDARY |

---

## Reference: Open GitHub Issues That Validate This Tool

Link these in posts to show demand:
- `anthropics/claude-code#8701` — Search conversation history in --resume
- `anthropics/claude-code#35599` — Support --resume latest
- `anthropics/claude-code#23954` — Picker keyboard navigation broken
- `anthropics/claude-code#29052` — Configurable session limit in /resume
- `anthropics/claude-code#47945` — Search sessions by UUID
- `anthropics/claude-code#11408` — Add ability to name and organize sessions
- `anthropics/claude-code#6907` — Auto-generate session summaries
- `anthropics/claude-code#24207` — No disk space management
- `anthropics/claude-code#32631` — Conversation branching

---

## Feature Roadmap (Post-Launch)

### v1.1 — The "Wow" Update
- [ ] Full-text content search across sessions (`Ctrl+/`)
- [ ] Per-session token count + cost estimate in picker
- [ ] Auto-generated display names for unnamed sessions

### v1.2 — Power User Features
- [ ] Git branch grouping
- [ ] One-key export to markdown (`Ctrl+E`)
- [ ] Fork tree visualization
- [ ] Disk usage display + cleanup

### v1.3 — Polish
- [ ] Session tagging (local sidecar file)
- [ ] Session templates / quick-start
- [ ] Shell keybinding integration (replace `claude --resume`)

---

## Newsletter Contacts

| Newsletter | Audience | How |
|------------|----------|-----|
| TLDR Tech | 1.25M+ | submissions@tldr.tech |
| Console.dev | Dev tools | console.dev/submit |
| Changelog | Open source | changelog.com/submit |
| Hacker Newsletter | 60K+ | Auto-curated from HN front page |
| daily.dev | Millions | Auto from Dev.to posts |

---

## Competitors Reference

| Tool | Language | Stars | Unique Feature | Our Advantage |
|------|----------|-------|---------------|---------------|
| claude-history | Rust | 197 | Full-text search, Vim viewer | No compile step, we add cost display |
| claude-sessions | TypeScript | - | Multi-agent, AI summaries | Lighter weight, fzf native |
| cc-sessions | Rust | - | Fork visualization | No Rust dependency |
| ccmanager | - | - | Multi-agent support | Focused on Claude, better UX |
| Claude Squad | Go | 6.6k | Parallel orchestration | Different category — we complement it |
