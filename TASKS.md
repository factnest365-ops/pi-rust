# TASKS.md — Pi → Tau Sew: L1/L2/L3 Router + Transparent Glass + Greater Think

> **HARNESS ACTIVE: L1/L2/L3 Router** — L1 `~/.pi/agent/models.json`, L2 `~/.pi/agent/models-store.json`, L3 `~/.pi/models_cache.json`
> **Boss intent:** Tau feels rigid/hardcoded/demo, model window ≠ Pi, wants *transparentish glass* (not matte opaque) + greater think. This TASKS.md is the single plan any agent can pursue solo.

---

## 0. Snapshot (2026-09-16)

- **Pi TS 0.80.10:** `173M` dist + `159M` node_modules, `56K` lines dist, 30 builtins, Ink TUI, `proper-lockfile` npm, JSON-on-disk themes (`dark.json` watched)
- **Tau pi-rust:** `2.2G` source, `37K` lines Rust, `9.8M` `tau`/`pi-rs` + `3.4M` `taud`, `<10M` RSS (vs `120M`), `<15ms` cold, `240` tests (695 total), `651` TODO/unwrap sauce, 7 crates
- **Catalog just fixed:** `openrouter/auto` `maxTokens:null` → `16384` (kilo 378 was 0 visible) → **L1 378 | L2 378 | L3 1483 total**, `ModelRuntime` error `none`, `CREATE 22ms`
- **Timeouts just fixed:** `httpIdle 10s→120s`, `retry off→on` (`3` retries, `provider 120s/2/30s`), `modelRefresh 15s→60s` (`dist/core/model-runtime.js`), `rust reqwest 5s→30s` (`models.rs`)
- **Reports:** `/tmp/pi-audit-report.md` + `/tmp/tau-audit-report.md` + `/tmp/tau-rigidity-report.md` (294 lines, 7/10 rigid)

---

## 1. Findings (audit synthesis)

### Pi strengths to sew
- JSON-on-disk + runtime registries + shared primitives: `fuzzyFilter`, `SelectList`, `theme watcher`, `extension loader`, `remote-catalog 4h`, `ModelRuntime snapshot+refresh`
- Scoped model picker (`all|scoped`), `defaultModelPerProvider` 35, `isAlias` logic, Inspector auth badges, CLI `list-models` context table

### Tau sauce (why rigid/demo)
- **Model window:** `model_picker.rs` 6-tab enum `All/Reasoning/Frontier/Local/Free/Configured` + `matches()` `contains("anthropic"|"openai"|…)` ×3 + `provider_color()` ladder `magenta/green/accent/cyan` + row/inspector dupe auth `AuthResolver::resolve_key`
- **Hardcoded inventory:** `provider_picker.rs` `ALL_PROVIDERS` 25 static entries, `style.rs` 8 themes `ThemeKind` enum + dead duplicate `Theme` (`#0a0a0a` vs `#0d1117`), `help_overlay.rs` 35 `&'static str` with stale `"33+/25+/7"` + apology comment for `&'static str` details, `plan_overlay.rs` badges `DarkGray/Yellow/Green/Red` bypass palette + `Space` theater `50/0/"Manual fail"`, `federation.rs` 3 personas `Jarvis/Friday/Ev` keyword ladder, `speculate.rs` 5 strategies + `spec-a/b` only race + `.tau/worktrees` literals, `models.rs` **75** `ModelInfo::new` + `infer_model_limits` 20-branch ladder + 11 sequential `if let Ok(res)` blocks + `auth.rs` 4 parallel alias/env/field/URL matches + `_ => Ok(true)` auto-pass
- **Demo feel:** `mermaid.rs` 477 lines + `TAU_LOGO_LINES` ASCII gate, Mermaid polish before palette fix, theme preview only for themes
- **Gaps:** `plugins.rs` 9 lines trait only, package-manager `rg-empty`, OAuth enum no flow, vault `limit=5` hardcoded

### Rigidity score
**Tau 7/10 rigid (3/10 flexible) vs Pi 4/10 rigid** — gap 3 points, all in chrome. Fix palette/help/registry → half gap before lunch.

---

## 2. Design Targets

### Transparentish Theme (not matte opaque)
Boss pref: **OLED Black Glass** `true #000000`, `85%` opacity `blur 30`, monochrome chrome borders `#181818`/`#242424`, `JetBrainsMono NFM`, 3-island glass capsules, **color only in code content**, keep `Winter Arc` in left pill, never add AI slop.

- **Glass islands:** Top bar + transcript + status bar + input = 3 glass capsules with `backdrop blur 30`, `bg rgba(0,0,0,0.85)`, border `1px #181818` outer / `#242424` inner, radius `12-14px`, shadow `0 8px 32px rgba(0,0,0,0.6)`
- **Not matte:** Pi TS uses `Ink` opaque; Tau ratatui currently `Color::Rgb(10,10,10)` opaque — replace with alpha glass + terminal transparency (Ghostty `background_opacity 0.85`, `blur` if macOS)
- **Monochrome chrome:** All UI chrome `Rgb(119,119,119)` muted, only syntax highlight colored (reuse Pi's monochrome rule)
- **Typography:** `JetBrainsMono Nerd Font Mono` everywhere, pill text `Winter Arc` left island
- **Implementation:** `style.rs` `ThemePalette` add `glassBg`, `glassBorder`, `glassBlur`; `pi-tui/src/style.rs` `default_pi` / `sketchybar` → glass; `model_picker.rs`/`provider_picker.rs`/`plan_overlay.rs`/`usage_overlay.rs`/`memory_overlay.rs:420` replace `Color::DarkGray/Yellow/Green/Red/Cyan/Black` literals with palette slots

### Greater Think
- **ThinkingLevelMap:** Wire `thinkingLevelMap {off:null, minimal, low, medium, high, xhigh, max}` per model (pi `claude-opus` etc `max→max`, `xhigh→xhigh`). Tau currently only `reasoning: bool` + `vision: bool`
- **Reasoning:** Surface `supports_reasoning` + `reasoningEffort` in picker badges + Inspector + `/think` switch, propagate `thinking` param to SSE (Anthropic `thinking`, OpenAI `reasoning_effort`)
- **Context budget bar:** Inspector `ctx/out` tokens already but add footer budget bar `used/total %` + color `Green<50 Yellow<80 Red` (currently `usage_overlay.rs` magic `25` width, palette-bypass)
- **Compaction think:** Summary prompt already dense factual, but add `thinkingLevel: high` for summary model to reduce hallucination

---

## 3. Tasks (agent-pursuable, smallest diff first)

### T-001 Palette bypass kill (30m) — P0
- **Files:** `crates/pi-tui/src/provider_picker.rs:183-253`, `plan_overlay.rs:158-161`, `usage_overlay.rs`, `memory_overlay.rs:420`, `style.rs`
- **Steps:** Delete dead `Theme` struct, extend `ThemePalette` with `glassBg/Border/Blur` + `highlightBg/Fg`, replace all `Color::DarkGray/Yellow/Green/Red/Cyan/Black` literals with palette fields, wire `theme_picker` preview to registry
- **Verify:** `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` + visual `pi-rs` model window no color drift
- **Done:** No hardcoded `Color::` outside `style.rs`

### T-002 Derive help counts (15m) — P0
- **Files:** `crates/pi-tui/src/help_overlay.rs`
- **Steps:** Change `HelpItem.details: &'static str → String/Cow`, interpolate `ALL_PROVIDERS.len()`, `ThemeKind::ALL.len()`, `catalog.len()`, `VERIFY_TIMEOUT_SECS`
- **Verify:** Add provider → help updates without edit; `rg "33\+|25\+|7 aesthetic"` 0 hits

### T-003 Table-drive taxonomy (1-2h) — P0
- **Files:** `crates/pi-tui/src/model_picker.rs:20-112`, new `crates/pi-tui/src/provider_registry.rs` or `crates/pi-providers/src/provider_meta.rs`
- **Steps:** Create `ProviderEntry { id, displayName, endpoint, frontier:bool, localPrefixes:&[], color: PaletteSlot }` table, `matches()` + `provider_color()` become lookups, remove duplicated `starts_with(ollama|…)` / `contains(free)` in list+inspector
- **Verify:** Add `provider: test/foo` → shows in correct tab/color with 1 edit; `search_models` still fuzzy

### T-004 Table-drive auth (2h) — P0
- **Files:** `crates/pi-providers/src/auth.rs`
- **Steps:** Single `ProviderAuth { aliases: &[&str], envKeys: &[&str], configField: &str, loginUrl: &str, testUrl: &str }` registry, collapse 4 matches (`provider_aliases`, `resolve_key`, `save_key`, `test_provider_key`, `get_login_instructions`), fix `_ => Ok(true)` → `Err("unknown provider")`
- **Verify:** `cargo test auth` + `pi --login unknown` fails closed

### T-005 Transparent glass theme (½d) — P0 Boss ask
- **Files:** `crates/pi-tui/src/style.rs`, `theme_picker.rs 179`, `crates/pi-tui/src/*.rs` (all overlays), `~/.config/ghostty/config`
- **Steps:** Add `ThemeKind::Custom` or `HashMap<String, ThemePalette>` layered over builtins, `~/.tau/themes/*.json` + `notify` watcher (mirror TS `initTheme` watcher), glass values `bg #000000 alpha 0.85 blur 30 border #181818/#242424`, monochrome chrome `119,119,119`, JetBrainsMono NFM, 3 islands, keep `Winter Arc` left pill
- **Verify:** `pi-rs` with glass vs matte screenshot diff, `ghostty` opacity 0.85 blur, theme file edit hot-reloads without restart

### T-006 Greater think wiring (½d) — P0 Boss ask
- **Files:** `crates/pi-providers/src/models.rs` (add `thinkingLevelMap`), `crates/pi-providers/src/lib.rs` (pass `thinking` to SSE), `crates/pi-tui/src/model_picker.rs` (badges), `crates/pi-core/src/lib.rs` (compaction summary with `high`)
- **Steps:** Port Pi's `thinkingLevelMap {off:null, xhigh→xhigh, max→max}` per model (from `@earendil-works/pi-ai` `models.generated`), surface `🧠` badge + Inspector `thinking` level, add `/think` slash + `Ctrl+T` cycle, propagate `thinkingLevel` to provider stream (`reasoning_effort`/`thinking.budget_tokens`), add footer budget bar `ctx/total %` palette-driven
- **Verify:** `kilo/claude-opus-4-5` shows `xhigh,max` cycle, `reasoning` true models show `🧠`, summary compaction uses `high`

### T-007 N-way speculate + strategy registry (½d) — P1
- **Files:** `crates/pi-core/src/speculate.rs`, `.tau/strategies/*.md`
- **Steps:** Generalize `run_speculative_race` over `strats[..]` (not just `strats[0]/[1]`), load strategies from `.tau/strategies/*.md`, promote `verify_workspace` defaults + `120s` to config, keep A-vs-B as default config
- **Verify:** 3-way race `spec-a/b/c` merges winner (fewest diff, then fastest)

### T-008 Plugin registry (1d) — P1
- **Files:** `crates/pi-tools/src/plugins.rs` (9 lines stub), `crates/pi-core/src/skills.rs` 288 lines, `crates/pi-cli/src/lib.rs`
- **Steps:** `inventory`/`linkme` `ToolPlugin` registry, `--no-plugins` flag parity with TS `--no-extensions/--no-skills`, project-trust gating, wire `SKILL.md` frontmatter `triggers/enable` + `disable-model-invocation`
- **Verify:** `cargo test` + `pi-rs --no-plugins` skips plugins, `SKILL.md` scan still works

### T-009 Kill duplicate default + theater (1h) — P1
- **Files:** `crates/pi-tui/src/style.rs` (delete legacy `Theme`), `crates/pi-tui/src/plan_overlay.rs:295-301`
- **Steps:** Delete legacy `Theme` (keep `ThemePalette::default_pi`), make plan Space require real verification result (stamp `summary` with command output, not `""`), progress bar width `20` → palette-driven, empty plan `progress 100%` fix
- **Verify:** Space on non-manual plan no-op without verification, diff viewer shows real output

### T-010 Catalog & timeout hardening (done, verify) — P0
- **Files:** `~/.pi/agent/models.json` (kilo 378), `~/.pi/agent/models-store.json` (7 providers), `~/.pi/models_cache.json` (1483), `dist/core/model-runtime.js` (60s), `crates/pi-providers/src/models.rs` (30s)
- **Steps:** Verify L1/L2/L3 still synced (`rg maxTokens:null` 0 hits), `ModelRuntime.create()` 22ms, `httpIdle 120s` + `retry on` + `provider 120s/2/30s`
- **Verify:** `node -e "import('...model-runtime.js').then(m=>m.ModelRuntime.create()).then(r=>console.log(r.getError()||'none', r.getModels().filter(x=>x.provider==='kilo').length))"` → `none 378`

### T-011 Lightweight sew verify (½d) — P0
- **Files:** `install.sh`, `Cargo.toml`
- **Steps:** `cargo build --release --bin tau --bin pi-rs`, `strip`, `du -sh`, `ps -o rss` `<10M`, `taud <4M`, cold `<15ms`, compare `173M+159M` vs `9.8M`
- **Verify:** `ls -lh ~/.tau/bin/*` + `cargo clippy -D warnings` + `cargo test --workspace -- --nocapture` `249` tests green

---

## 4. Execution Order (leverage)

`T-001 → T-002 → T-003 → T-005 → T-006` kills demo feel before lunch. `T-004, T-007, T-008, T-009` afternoon. `T-010, T-011` verification. Vault untouched (least rigid).

## 5. Harness Notes

- **Router L1/L2/L3:** Any model add must land in `models.json` L1 → `models-store` L2 (checkedAt) → `models_cache` L3 (last_refreshed) — `python3 /tmp/full_catalog_sync.py` pattern.
- **Transparent SOP:** No new opaque matte theme, no AI slop, keep Winter Arc pill, JetBrainsMono NFM, 85% blur 30.
- **Think SOP:** Every reasoning model must expose `thinkingLevelMap`, Inspector badge, and footer budget bar.

---

## 6. Done Criteria (for any agent)

- `cargo clippy --workspace --all-targets -- -D warnings` 0 warnings
- `cargo test --workspace -- --nocapture` 100% (197/197 + 249 total)
- `rg "Color::|33\+ providers|25\+ gateways|7 aesthetic|intentionally static"` 0 hits outside `style.rs`
- `pi-rs` model window: fuzzy, 6 tabs data-driven, provider color palette, derived help counts, glass islands 85% blur 30, `Winter Arc` left
- `/tmp/tau-rigidity-report.md` re-run → Rigidity `7/10 → 4/10`
