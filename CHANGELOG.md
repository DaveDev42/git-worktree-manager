# Changelog

## [0.1.6](https://github.com/DaveDev42/git-worktree-manager/compare/v0.1.5...v0.1.6) (2026-05-11)


### Bug Fixes

* **cli:** reject gw --prompt/--prompt-file leaked into forward args ([#157](https://github.com/DaveDev42/git-worktree-manager/issues/157)) ([05a83b6](https://github.com/DaveDev42/git-worktree-manager/commit/05a83b60411401a2367a60f167b6cd38bf234f25))

## [0.1.5](https://github.com/DaveDev42/git-worktree-manager/compare/v0.1.4...v0.1.5) (2026-05-06)


### Bug Fixes

* **env:** strip CLAUDE_CODE_ENTRYPOINT/EXECPATH from forwarded env to keep launched claude in TUI mode ([#154](https://github.com/DaveDev42/git-worktree-manager/issues/154)) ([ecd8aa7](https://github.com/DaveDev42/git-worktree-manager/commit/ecd8aa7a4855b02937699bda83e3a262dc065e86))

## [0.1.4](https://github.com/DaveDev42/git-worktree-manager/compare/v0.1.3...v0.1.4) (2026-05-06)


### Features

* forward args to AI tool, drop --no-term/--prompt-stdin, add env auto-snapshot ([#148](https://github.com/DaveDev42/git-worktree-manager/issues/148)) ([6726284](https://github.com/DaveDev42/git-worktree-manager/commit/67262844b3a73dae01e7cf97e09eda1236382d23))
* **guard:** inject PreToolUse(Bash) hook into gw-spawned Claude sessions ([#147](https://github.com/DaveDev42/git-worktree-manager/issues/147)) ([b12f4f8](https://github.com/DaveDev42/git-worktree-manager/commit/b12f4f829b7af069bbe7868165b6fbc0c8ee772f))


### Bug Fixes

* lift hyphen-led target into forward_args for spawn/resume ([#149](https://github.com/DaveDev42/git-worktree-manager/issues/149)) ([fa6b9e6](https://github.com/DaveDev42/git-worktree-manager/commit/fa6b9e68fa5ac9a04fc660b1885af84f0aa70275))
* **resume:** always inject resume flag, drop session-detection gate ([#150](https://github.com/DaveDev42/git-worktree-manager/issues/150)) ([5fef11b](https://github.com/DaveDev42/git-worktree-manager/commit/5fef11b14483ebb3349d1eb24557c38c17718df3))
* **spawn:** use interactive prompt for gw new/spawn --prompt ([#143](https://github.com/DaveDev42/git-worktree-manager/issues/143)) ([e9dcab9](https://github.com/DaveDev42/git-worktree-manager/commit/e9dcab996c4a7549710a13efeb5d8d511920d5e2))

## [0.1.3](https://github.com/DaveDev42/git-worktree-manager/compare/v0.1.2...v0.1.3) (2026-05-03)


### Bug Fixes

* **spawn-ai:** use absolute self-exe path and escape iTerm AppleScript ([#140](https://github.com/DaveDev42/git-worktree-manager/issues/140)) ([e447c94](https://github.com/DaveDev42/git-worktree-manager/commit/e447c944af7fcd3e10d19c3cff9f049a575a84ac))

## [0.1.2](https://github.com/DaveDev42/git-worktree-manager/compare/v0.1.1...v0.1.2) (2026-05-02)


### Features

* restore -T/--term per-invocation launcher override ([#138](https://github.com/DaveDev42/git-worktree-manager/issues/138)) ([d60cd52](https://github.com/DaveDev42/git-worktree-manager/commit/d60cd528e8b3353b3b89374fedeaf6200f641627))

## [0.1.1](https://github.com/DaveDev42/git-worktree-manager/compare/v0.1.0...v0.1.1) (2026-05-02)


### Bug Fixes

* **release:** gate downstream jobs on resolve-release success ([#135](https://github.com/DaveDev42/git-worktree-manager/issues/135)) ([36fd4a9](https://github.com/DaveDev42/git-worktree-manager/commit/36fd4a9a168d3051068ac29a425b1018b288b42b))

## [0.1.0](https://github.com/DaveDev42/git-worktree-manager/compare/v0.1.0...v0.1.0) (2026-05-02)


### Features

* acquire session lockfile when entering gw shell ([24da17e](https://github.com/DaveDev42/git-worktree-manager/commit/24da17e2fbe15d93e982b3cd1b98f72f3e4466ef))
* acquire session lockfile when launching AI tools ([a0a2a34](https://github.com/DaveDev42/git-worktree-manager/commit/a0a2a34074f8a5aa9049fb46e21249bda179a5a3))
* add busy detection with lockfile + process cwd scan ([102435d](https://github.com/DaveDev42/git-worktree-manager/commit/102435dc4334c868f6f38b09ac07026b04bf1abd))
* add busy status to worktree state with highest non-stale priority ([ae18947](https://github.com/DaveDev42/git-worktree-manager/commit/ae189470d1d74a5cc22ab1e1eeac9da96411dda8))
* add Claude Code skill for worktree task delegation ([52f0d06](https://github.com/DaveDev42/git-worktree-manager/commit/52f0d06d24fb258780c1c7e1549cfacda50cdf94))
* add Claude Code skill for worktree task delegation ([478619b](https://github.com/DaveDev42/git-worktree-manager/commit/478619bf4d95389f473feaed594a3a8ef407ae5a))
* add dynamic branch name tab completion for all subcommands ([b2d895c](https://github.com/DaveDev42/git-worktree-manager/commit/b2d895cd3c9cf91e40a98d271964b73bc567dc39))
* add get_pr_state helper to query GitHub PR status via gh CLI ([d217d5f](https://github.com/DaveDev42/git-worktree-manager/commit/d217d5febbf0b5357b7adb8d9ddfe728838c3fd3))
* add icon and color mappings for merged and pr-open worktree statuses ([c917728](https://github.com/DaveDev42/git-worktree-manager/commit/c91772897ed5ac324e51a6e8287a8c97b018035a))
* add merged and pr-open worktree statuses ([3096081](https://github.com/DaveDev42/git-worktree-manager/commit/3096081667e46d52e3a39029d00fa05aaee911c2))
* add session lockfile with RAII guard and PID liveness check ([2fc68aa](https://github.com/DaveDev42/git-worktree-manager/commit/2fc68aa83f922afc7beea205e4ead609e2e6b35f))
* add SHA256 checksums to release artifacts ([d70336e](https://github.com/DaveDev42/git-worktree-manager/commit/d70336ef9aada8ec58e0c66f53ed873342306bea))
* add WezTerm background tab (w-t-b) + fix iTerm AppleScript ([dd69f47](https://github.com/DaveDev42/git-worktree-manager/commit/dd69f47fac6db0ba768c64a7992244a415aa3f4a))
* add WezTerm background tab launch method (w-t-b) ([cac993c](https://github.com/DaveDev42/git-worktree-manager/commit/cac993ce8af789d1825995782b4de4896223b0d8))
* auto-detect repository default branch ([2e4321c](https://github.com/DaveDev42/git-worktree-manager/commit/2e4321c62c5d4a1274eece01b901c599fcf16a8b))
* auto-upgrade via self-replace from GitHub Releases ([48b948e](https://github.com/DaveDev42/git-worktree-manager/commit/48b948e7a61ebb2826f7006ea19ee5f47b2beb76))
* **busy:** detect_busy_tiered combines Claude session + lockfile (Hard) and scan (Soft) ([45d5f9c](https://github.com/DaveDev42/git-worktree-manager/commit/45d5f9c40dd3520ffafb465b4a779171e59662cb))
* **busy:** refusal message renderer for tiered busy model ([3d717db](https://github.com/DaveDev42/git-worktree-manager/commit/3d717db227054878a045611aa96980f3053970a9))
* centralize messages and add format_age tests ([b675c83](https://github.com/DaveDev42/git-worktree-manager/commit/b675c83e085dabfc2548c669d7856c18714870d6))
* **claude_session:** find_active_sessions with threshold + cwd filter ([aef7172](https://github.com/DaveDev42/git-worktree-manager/commit/aef7172d4d9848ac45c0759f789dff7a16a12f0b))
* **claude_session:** newest_event_timestamp / cwd jsonl tail parsers ([7b74c8e](https://github.com/DaveDev42/git-worktree-manager/commit/7b74c8e4379e7885cba812e9af4e047fe1fcd825))
* **claude_session:** path encoding for ~/.claude/projects/ lookup ([dbcc655](https://github.com/DaveDev42/git-worktree-manager/commit/dbcc655ed7363a4203ed8b31a9042884955622ef))
* **clean:** skip busy worktrees and report summary, --force to override ([1fc20ee](https://github.com/DaveDev42/git-worktree-manager/commit/1fc20ee3fcabb7aef910ee4831b75e098a3f373e))
* **cli:** --no-cache on clean and global; parallelize TODO; rename pr_state ([f324ae6](https://github.com/DaveDev42/git-worktree-manager/commit/f324ae600c2cb762e8bfe469caca5357091a6a54))
* **cli:** accept multiple delete targets and -i/--dry-run flags ([320a472](https://github.com/DaveDev42/git-worktree-manager/commit/320a472ffbef195c49b1bcb50831413c64640ce7))
* **cli:** add --no-cache flag to list subcommand ([4b96b17](https://github.com/DaveDev42/git-worktree-manager/commit/4b96b1731017cc4ce6c6a8a1f68bc1a283d2286e))
* **cli:** add --prompt-file and --prompt-stdin to gw new ([30f8086](https://github.com/DaveDev42/git-worktree-manager/commit/30f80861c1fc53349d470d220cba0194be2a8722))
* **cli:** add --prompt-file and --prompt-stdin to gw new ([0425c7a](https://github.com/DaveDev42/git-worktree-manager/commit/0425c7a25f567ed10fb6a348bb4287d5dda45423))
* **cli:** resolve prompt from --prompt/--prompt-file/--prompt-stdin ([235fac2](https://github.com/DaveDev42/git-worktree-manager/commit/235fac24e0130410fe0340d268256631fe5dd548))
* code quality — extract helpers, split doctor, shell completion prompt ([faa6eba](https://github.com/DaveDev42/git-worktree-manager/commit/faa6ebad0c4984a24bda9bf8ed79055c4a4f17df))
* code quality refactoring, shell completion parity, CI improvements ([e00938c](https://github.com/DaveDev42/git-worktree-manager/commit/e00938cadad1d214c71c0580769dc1ac7a206293))
* Complete feature parity — all missing commands and options ([908b81c](https://github.com/DaveDev42/git-worktree-manager/commit/908b81c461db3195d3f9d2db949a5d4251c1d169))
* Complete Python feature parity — all 15 items implemented ([e4d86ba](https://github.com/DaveDev42/git-worktree-manager/commit/e4d86babb8b58017db342f3145fe1d06a5eac57b))
* config list table, config key tab completion, cw bash completion ([8ad734f](https://github.com/DaveDev42/git-worktree-manager/commit/8ad734f14dd736d693532f1c47b42a98fcf3f2c2))
* **delete_batch:** add plan types and resolver ([ef2e834](https://github.com/DaveDev42/git-worktree-manager/commit/ef2e8343efdd9177946dcf6de83abb266d431d85))
* **delete_batch:** add summary printer and batch confirmation ([908ca1d](https://github.com/DaveDev42/git-worktree-manager/commit/908ca1dc28dde40ef2362029de0c1a83bd313489))
* **delete:** block busy worktree deletion with TTY-aware prompt ([9d33e08](https://github.com/DaveDev42/git-worktree-manager/commit/9d33e08a46a62cbbd918487d2e4dc238c40b51fa))
* **delete:** multi-target support with -i and --dry-run ([6713359](https://github.com/DaveDev42/git-worktree-manager/commit/6713359e4aea19de098e1b7638e093d63a54c53e))
* **delete:** orchestrate multi-target delete with dry-run and batch confirm ([d186c5a](https://github.com/DaveDev42/git-worktree-manager/commit/d186c5a87fe4e70ac68521d8a46d089a22abd01d))
* **delete:** tiered refusal in batch + clean paths ([a247dd8](https://github.com/DaveDev42/git-worktree-manager/commit/a247dd8b6994df406ebb1f2ff35706eb28356472))
* **delete:** tiered refusal messages, drop interactive y/N prompt ([e3f41ba](https://github.com/DaveDev42/git-worktree-manager/commit/e3f41ba2a5a9340506cf6bfdf67175edcc7b09e0))
* **display,tests:** show busy in tree/stats UI; robust busy integration tests ([a9b69b0](https://github.com/DaveDev42/git-worktree-manager/commit/a9b69b099acef86aee0d064901f34121ce09c1e0))
* **doctor:** --session-start --quiet single-line hook-friendly mode ([0deecda](https://github.com/DaveDev42/git-worktree-manager/commit/0deecda364131e5cd4deff2a45cfac563e2f9f83))
* **doctor:** detect plugin install + suggest upgrade from legacy skill ([2f031e9](https://github.com/DaveDev42/git-worktree-manager/commit/2f031e9efd3672cbe1d09a4360943181aa0bbcce))
* expand skill with full gw reference and references/ support ([aa1a9c1](https://github.com/DaveDev42/git-worktree-manager/commit/aa1a9c123b89b5f3000af3ddaba0434bab2a4b39))
* **guard:** hook helper to block risky bash in unhealthy cwd ([1d17d58](https://github.com/DaveDev42/git-worktree-manager/commit/1d17d584140a09e7268ead58359bbb49d6c9b051))
* gw plugin + worktree-health skill + tiered in-use detection ([513645d](https://github.com/DaveDev42/git-worktree-manager/commit/513645d0b0d5e9b859eac70ce681fbcb3c332b44))
* hybrid worktree busy detection for delete/clean ([1abcb24](https://github.com/DaveDev42/git-worktree-manager/commit/1abcb245dc7f3620c42c63d315c09ecc12bc7bb6))
* improve tab completion for all option values ([c5bb0d0](https://github.com/DaveDev42/git-worktree-manager/commit/c5bb0d08ed1e66b0d54d1086ed9731cf286c2194))
* improve tab completion for all option values ([08dc42a](https://github.com/DaveDev42/git-worktree-manager/commit/08dc42a8dafdc1e527a2fb466067f6f59785c527))
* integrate merged and pr-open statuses into worktree display ([ccd518e](https://github.com/DaveDev42/git-worktree-manager/commit/ccd518e15cb24d7b75fc8c3b999956de112ffff9))
* **list:** progressive Inline Viewport rendering in TTY mode ([67bae1d](https://github.com/DaveDev42/git-worktree-manager/commit/67bae1d7f5f2232c3429623eb768c9b37f6ba08f))
* **list:** restore terminal on err, early-return on empty worktree set ([268758f](https://github.com/DaveDev42/git-worktree-manager/commit/268758f996ebc2084fa67a15a3fe5f0d83aafee0))
* Phase 1 — project scaffolding with core infrastructure ([8bdeb88](https://github.com/DaveDev42/git-worktree-manager/commit/8bdeb887aba1b69c6627ed4d67a9a1e927142eb6))
* Phase 2 — core commands (new, delete, merge, pr, sync, config) ([c310fe6](https://github.com/DaveDev42/git-worktree-manager/commit/c310fe65fc0c8bc198fa4a06f2740517d9021369))
* Phase 3 — terminal launchers and AI tool integration ([5e9d07b](https://github.com/DaveDev42/git-worktree-manager/commit/5e9d07bb3120732bc00d4d78f457f025237db59f))
* Phase 4 — shell functions, auto-update, and upgrade ([e99b7c7](https://github.com/DaveDev42/git-worktree-manager/commit/e99b7c7aeb2424fdf4b58964e06553c00593a817))
* Phase 5 — backup, diagnostics, CI/CD, and polish ([983bdc3](https://github.com/DaveDev42/git-worktree-manager/commit/983bdc37ed8a73fef02bc61ffac76ea8ef637bd7))
* **pr_cache:** add module skeleton with repo-hash and cache path ([85dd3b7](https://github.com/DaveDev42/git-worktree-manager/commit/85dd3b7b69b7d1985c28a1de0ac3aa559a3fdc3a))
* **pr_cache:** fetch PR state from gh in one batched call ([3bfbe53](https://github.com/DaveDev42/git-worktree-manager/commit/3bfbe53e40b35151f1b6d959452aec950087cd1d))
* **pr_cache:** persist PR state to disk with 60s TTL ([2c7699b](https://github.com/DaveDev42/git-worktree-manager/commit/2c7699ba3f0cee9f3a7b9ecd84bf2e7ce8d1309e))
* **pr_cache:** public load_or_fetch with --no-cache semantics ([f61b3dd](https://github.com/DaveDev42/git-worktree-manager/commit/f61b3dd9265df6781ea6ab977fc175d253357ebf))
* Python parity improvements — CLI flags, config fallback, colored hooks ([a3c8ee2](https://github.com/DaveDev42/git-worktree-manager/commit/a3c8ee2021bd7102a1db8ba8e4f35c875e611726))
* QA improvements — config get, delete --force, hook aliases, backup filtering ([b8a7cc0](https://github.com/DaveDev42/git-worktree-manager/commit/b8a7cc048b4593f935646d21245cccd1cd09dc67))
* redesign — worktree-first surface, mr-style scope discovery ([#130](https://github.com/DaveDev42/git-worktree-manager/issues/130)) ([c897059](https://github.com/DaveDev42/git-worktree-manager/commit/c8970591875528bda660a2b08a255217c18aada6))
* refresh shell cache on gw shell-setup ([eebde54](https://github.com/DaveDev42/git-worktree-manager/commit/eebde549cb89446451aa261fe29d0ad45680c504))
* register gw/cw tab completion in shell functions ([1eab90d](https://github.com/DaveDev42/git-worktree-manager/commit/1eab90d615edb5d5803a4fcc854968f1b7bcd740))
* rename gw-delegate skill to gw with natural language support ([30f2012](https://github.com/DaveDev42/git-worktree-manager/commit/30f20122c226c6fd7264fc4e7a86efb2eb46346c))
* rename gw-delegate skill to gw with natural language support ([dfe1fcb](https://github.com/DaveDev42/git-worktree-manager/commit/dfe1fcb0ba43cfa01726813a466040104c585595))
* Rename to git-worktree-manager (gw) ([3c061f1](https://github.com/DaveDev42/git-worktree-manager/commit/3c061f1d426eda942664f770ef81fa1a0e4d0aaa))
* rich TUI output, duration parsing, hook error formatting ([7047b87](https://github.com/DaveDev42/git-worktree-manager/commit/7047b873c1175315156613b61d9ff42da27b9c9b))
* **setup_claude:** manage skill body — guidance + rulebook + hook catalog ([89b7490](https://github.com/DaveDev42/git-worktree-manager/commit/89b7490989d17bc1f42a6ebb979884978f8b3dfa))
* **setup_claude:** migrate existing skill body into delegate skill ([94bfe0f](https://github.com/DaveDev42/git-worktree-manager/commit/94bfe0fb5fe64cd1425225fc010e9ec04537082b))
* **setup-claude:** harden plugin with pre-flight + busy-sibling rules ([#129](https://github.com/DaveDev42/git-worktree-manager/issues/129)) ([ba6432b](https://github.com/DaveDev42/git-worktree-manager/commit/ba6432bb1a3ec1d505cf1020033712b0581df1c4))
* show launch method display names in config show/list ([b0f552f](https://github.com/DaveDev42/git-worktree-manager/commit/b0f552f9ed35a93ae2658fb77ea23f6183b97cdb))
* smart config set for ai_tool and launch.method ([79f2102](https://github.com/DaveDev42/git-worktree-manager/commit/79f21026a5a742fe92c739543848e03b5a2c63e7))
* **spawn_spec:** add 24h sweep_stale for crash-residue cleanup ([4516521](https://github.com/DaveDev42/git-worktree-manager/commit/451652144661d04b3916f3dd32903281e9be3637))
* **spawn_spec:** implement execute reading spec and execvp-ing target ([865f766](https://github.com/DaveDev42/git-worktree-manager/commit/865f76604fae8f7aedea23ea72b4d89a639b6240))
* **spawn_spec:** implement materialize writing 0600 tempfile ([6166cbb](https://github.com/DaveDev42/git-worktree-manager/commit/6166cbb712fe1f60c481ee199b10df11444bd0aa))
* **spawn_spec:** scaffold SpawnSpec struct with round-trip test ([be10b65](https://github.com/DaveDev42/git-worktree-manager/commit/be10b659f05b512d1ad9f87b08fd4fe5e36c185c))
* **spawn-ai:** enable no-arg recovery from corrupted launcher line ([#125](https://github.com/DaveDev42/git-worktree-manager/issues/125)) ([48e3c4f](https://github.com/DaveDev42/git-worktree-manager/commit/48e3c4fbbaf2831f462d444345c933f007440a44))
* suggest .claude/settings.local.json in .cwshare setup ([1e742ba](https://github.com/DaveDev42/git-worktree-manager/commit/1e742ba5cf643daf4e8f634c3f27b71b720dff9f))
* suggest .claude/settings.local.json in .cwshare setup ([4594ef6](https://github.com/DaveDev42/git-worktree-manager/commit/4594ef6ccaeaf0bf819686d3d53f24d24482cea7))
* **tui:** add multi-select widget and wire gw delete -i ([b4b37f1](https://github.com/DaveDev42/git-worktree-manager/commit/b4b37f1585046c251ef7de6d03698dca116730da))
* **tui:** Inline Viewport app skeleton with snapshot tests ([8084246](https://github.com/DaveDev42/git-worktree-manager/commit/8084246b7cdafedc9870bc02c1823842501a234f))
* **tui:** progressive render loop consuming mpsc updates ([6f4f52a](https://github.com/DaveDev42/git-worktree-manager/commit/6f4f52a25772773da9dae09be8bde87172a1a05e))
* **tui:** promote tui to directory module, add style palette and panic hook ([c5e6855](https://github.com/DaveDev42/git-worktree-manager/commit/c5e685529884ca3f0e930827cc2600421b9a85f0))
* **tui:** show relative age and busy badge in delete -i selector ([#116](https://github.com/DaveDev42/git-worktree-manager/issues/116)) ([dea9e43](https://github.com/DaveDev42/git-worktree-manager/commit/dea9e43ca5271e2c06d11a65f27c820675126f78))
* **upgrade:** add --yes flag for non-interactive upgrade ([#104](https://github.com/DaveDev42/git-worktree-manager/issues/104)) ([5ada374](https://github.com/DaveDev42/git-worktree-manager/commit/5ada374156ddb7a859848a004ae32b3aa21c6c20))
* use self_update crate for upgrade, detect Homebrew installs ([eb6aff4](https://github.com/DaveDev42/git-worktree-manager/commit/eb6aff48025f55d03a78213ac861cfe2924cc824))


### Bug Fixes

* add --fail to version check curl and validate curl in PATH ([b062de8](https://github.com/DaveDev42/git-worktree-manager/commit/b062de8ff4a95e77c88263349df9b8e300a51da5))
* add curl PATH check to fetch_latest_version for consistency ([88c50a6](https://github.com/DaveDev42/git-worktree-manager/commit/88c50a6672057c4b61627bef3d7fcb6a7837ccd4))
* Add cw-cd backward compatibility alias ([#9](https://github.com/DaveDev42/git-worktree-manager/issues/9)) ([f2a1361](https://github.com/DaveDev42/git-worktree-manager/commit/f2a13615848dc8883e1cad56351b4e337ab30b13))
* **ai-tools:** eliminate shell-escape failures via gw _spawn-ai wrapper ([567e129](https://github.com/DaveDev42/git-worktree-manager/commit/567e129ce678ed8c02cf6e61c95cbe3f9edbcfce))
* **ai-tools:** route AI spawn through gw _spawn-ai wrapper ([b6ca8a2](https://github.com/DaveDev42/git-worktree-manager/commit/b6ca8a2f0e1845ca8a6542c7d5726a7fa29a252a))
* auto-update Homebrew formula on release ([517eefa](https://github.com/DaveDev42/git-worktree-manager/commit/517eefac6081e57780767a8c0add520e66dc79db))
* auto-update Homebrew formula on release ([b40b091](https://github.com/DaveDev42/git-worktree-manager/commit/b40b09117bf4d6fdde8938951b8dd83a1bfceaa5))
* **backup:** show main-worktree backups and sanitize slash-in-branch ([9ea6c6e](https://github.com/DaveDev42/git-worktree-manager/commit/9ea6c6ea2885c5e8850ea72adf8fceee61d84fe5))
* bump MSRV to 1.85, fix rustfmt formatting ([e59ff8e](https://github.com/DaveDev42/git-worktree-manager/commit/e59ff8ec11f28d88175f4b5867d09377c21a13eb))
* **busy-detection:** address review pass 2 issues ([4e2e9bb](https://github.com/DaveDev42/git-worktree-manager/commit/4e2e9bb7b171db132e66aa67c7384cc6941e14a6))
* **busy:** broaden multiplexer match + disable lsof truncation ([22f7778](https://github.com/DaveDev42/git-worktree-manager/commit/22f777855cfc3af80e258495f7500281a08670be))
* **busy:** clean up process name and pgid-based sibling exclusion ([92d6d5b](https://github.com/DaveDev42/git-worktree-manager/commit/92d6d5ba87a6eeb302c92ab2904e4483d6f3817b))
* **busy:** exclude terminal multiplexer servers from cwd scan ([5711658](https://github.com/DaveDev42/git-worktree-manager/commit/571165893a872731767827330b0ab6d95d13808a))
* **busy:** local Command import in scan_cwd test so Linux compiles ([feb3472](https://github.com/DaveDev42/git-worktree-manager/commit/feb347278a2bf5cbae0d7cd2cc52c89067f2c217))
* **busy:** narrow Command import to macos; Linux uses /proc ([4687770](https://github.com/DaveDev42/git-worktree-manager/commit/4687770a43dbea42a5605d2fe2d8306674100d75))
* **busy:** require live claude process to flag worktree busy ([#112](https://github.com/DaveDev42/git-worktree-manager/issues/112)) ([49a622a](https://github.com/DaveDev42/git-worktree-manager/commit/49a622a33ccdd2f328f73d4ee4230d6a31146c09))
* **busy:** resolve argv-rewritten cmd and exclude pipeline siblings ([6d2e957](https://github.com/DaveDev42/git-worktree-manager/commit/6d2e957a6537c645f30473829a6c86561a9263ed))
* **busy:** verify cwd is inside target on macOS lsof parse ([cfe0346](https://github.com/DaveDev42/git-worktree-manager/commit/cfe03464e7cebff5c7d89a63cdbd33cb47483351))
* check GITHUB_TOKEN/GH_TOKEN env before spawning gh CLI ([e62c3f7](https://github.com/DaveDev42/git-worktree-manager/commit/e62c3f78d87bce74fe7e55f9d756514fd092b9be))
* **clean:** use PrCache for --merged detection to handle squash merges ([#109](https://github.com/DaveDev42/git-worktree-manager/issues/109)) ([ee80a26](https://github.com/DaveDev42/git-worktree-manager/commit/ee80a26d4507714340142f45955d9d0536bf0806))
* companion binary overwritten with old version during upgrade ([c6d4334](https://github.com/DaveDev42/git-worktree-manager/commit/c6d43341c1e1ae16f7bec10783c56d81578601d2))
* companion binary overwritten with old version during upgrade ([505f4d9](https://github.com/DaveDev42/git-worktree-manager/commit/505f4d9dc3c38662ea3df2b038ee09fc4aebebfd))
* create draft releases until binaries are uploaded ([319829c](https://github.com/DaveDev42/git-worktree-manager/commit/319829cdcd51f9a73b4e6718e850bcaab385c537))
* create GitHub releases as draft until binaries are uploaded ([4b5cb45](https://github.com/DaveDev42/git-worktree-manager/commit/4b5cb4589c8f72e990fdc55d46ecdd5cc970c800))
* critical busy-detection bugs found during smoke testing ([6fe35d8](https://github.com/DaveDev42/git-worktree-manager/commit/6fe35d83703ee981f4383ddec29c6800c7b19c0f))
* **delete_batch:** distinguish Nothing-selected from Cancelled in -i flow ([de39b00](https://github.com/DaveDev42/git-worktree-manager/commit/de39b009699efc93d7ecb08b304842f8b289687a))
* **delete_batch:** route summary to stdout and add dry-run trailer ([ca85db6](https://github.com/DaveDev42/git-worktree-manager/commit/ca85db6f752ffbbae7421dcd75e32e72cd92a6ba))
* **delete:** use explicit --force as busy override, not default git-force flag ([02a8a4e](https://github.com/DaveDev42/git-worktree-manager/commit/02a8a4ecadc459fc6af70d9c1d31e0d7074232eb))
* **deps:** bump ratatui 0.28.1 → 0.30.0 and migrate TUI code ([c1d3035](https://github.com/DaveDev42/git-worktree-manager/commit/c1d3035aa1766002a84c26375746cf5e8a1a02fa))
* **deps:** bump ratatui to 0.30 ([befe0e7](https://github.com/DaveDev42/git-worktree-manager/commit/befe0e785e476966753e045b0ec26223b061c066))
* disable native-tls in self_update to fix cross-compilation ([cf359f5](https://github.com/DaveDev42/git-worktree-manager/commit/cf359f5cf7220775b398f557f9ce72d27e500b5d))
* disable zip default-features to fix MSRV 1.85 and remove time vulnerability ([78ffb28](https://github.com/DaveDev42/git-worktree-manager/commit/78ffb283c609c566d2d50d71adb101ce1db261de))
* extract testable pure function and add unit tests for tab lookup ([a9f9335](https://github.com/DaveDev42/git-worktree-manager/commit/a9f93358a03aa70f34576038dc7278e798b38b70))
* improve curl error message and add User-Agent to version check ([6b1bd42](https://github.com/DaveDev42/git-worktree-manager/commit/6b1bd425366fdf0f506188220440f1538147ff1a))
* **list:** apply all 52 review items — correctness, hygiene, API, polish, docs ([3d7a942](https://github.com/DaveDev42/git-worktree-manager/commit/3d7a942c04a59c18e835273c26ea03d7c986d8f1))
* **list:** correctness, behavior polish, docs, tests — apply pass-2 items A/B/D/E ([ec2e0a9](https://github.com/DaveDev42/git-worktree-manager/commit/ec2e0a95f8a51c9a522e6598e25b68b06b0affde))
* **list:** panic-safe sweep, in_place_scope, extract footer helper ([d4b64d2](https://github.com/DaveDev42/git-worktree-manager/commit/d4b64d2ec9152d6bb15f852ec72f4b1a43daf1aa))
* **list:** pass-3 correctness — atomic flag ordering, panic readability, ratatui setup race ([0d074e8](https://github.com/DaveDev42/git-worktree-manager/commit/0d074e8a094d388277a1abef93cfa8319eab9bb4))
* **list:** pass-4 — env safety, prefetch filter, exhaustive PrState, comment fixes ([1e91d72](https://github.com/DaveDev42/git-worktree-manager/commit/1e91d727cb9b09063ee772f0fe78695c0a7b7451))
* **list:** pass-5 final — sweep scope, panic message clarity, test rigor ([89d472e](https://github.com/DaveDev42/git-worktree-manager/commit/89d472ec6f4b043d14390be7d7563551ba696bb3))
* **lockfile:** cfg-gate STALE_TTL so unix build does not warn ([44469dd](https://github.com/DaveDev42/git-worktree-manager/commit/44469ddc74eef095d35f0264c72de194fbbb0fea))
* **lockfile:** post-rename ownership check, thiserror, tmp cleanup sweep, version field ([41d1ef9](https://github.com/DaveDev42/git-worktree-manager/commit/41d1ef99036bca39aea4dbebded894e049b59002))
* pin time and base64ct for MSRV 1.85 compatibility ([edf490d](https://github.com/DaveDev42/git-worktree-manager/commit/edf490dbbe8e4e72addb949b79e64234de786c21))
* prevent env var race condition and config leakage in launch method tests ([378c4eb](https://github.com/DaveDev42/git-worktree-manager/commit/378c4ebd5435d118b4f7b3471b81fc0a49b86fdf))
* prevent env var race condition in launch method tests ([cf0e62f](https://github.com/DaveDev42/git-worktree-manager/commit/cf0e62fef81c03f50dd8f5986f89b095db63de9c))
* remove invalid [warnings] section from audit.toml ([67433d3](https://github.com/DaveDev42/git-worktree-manager/commit/67433d316de723256e3fc503cf3516a6dd93d91c))
* replace self_update with reqwest[rustls-native-roots] for enterprise MDM TLS support ([cb12a30](https://github.com/DaveDev42/git-worktree-manager/commit/cb12a30aff4315857460e577572e0a8fcc2dbd81))
* restore audit.toml ignores for zip→time and indicatif→number_prefix ([ec6ceed](https://github.com/DaveDev42/git-worktree-manager/commit/ec6ceedf0c71d5e8644af8cc1a10ebd1cbedc39a))
* restore focus to active tab instead of calling tab in w-t-b launcher ([3354dc1](https://github.com/DaveDev42/git-worktree-manager/commit/3354dc1dc291e154996a7986b9ec6aa935d2daea))
* restore focus to active tab instead of calling tab in w-t-b launcher ([f785abd](https://github.com/DaveDev42/git-worktree-manager/commit/f785abd5c7b87e477058f70def166288878cccb6))
* revert version to 0.0.1 so Release Please creates v0.0.2 ([addd37f](https://github.com/DaveDev42/git-worktree-manager/commit/addd37f57d33d8bab48fe5583411dfdc2df1aa96))
* **setup_claude:** add setup_claude_under(home) so tests can install hermetically on Windows ([44f1bc1](https://github.com/DaveDev42/git-worktree-manager/commit/44f1bc1a261577fbbc41f2f62fcf3eec5ec1e747))
* **setup-claude:** install plugin via local Claude Code marketplace ([#107](https://github.com/DaveDev42/git-worktree-manager/issues/107)) ([2c9370e](https://github.com/DaveDev42/git-worktree-manager/commit/2c9370e47d4c8c5c8d591b79727e8249b1413e41))
* skip Unix-only tests on Windows ([79ce973](https://github.com/DaveDev42/git-worktree-manager/commit/79ce973f96902d9351ebea05d3edbfb6d6bda24a))
* **spawn_spec:** address review feedback (exit 127, TOCTOU, spec alignment) ([9faa657](https://github.com/DaveDev42/git-worktree-manager/commit/9faa65790be7369c61eb561df9439e2a80f2ced6))
* **spawn_spec:** drop exec so launching shell survives AI exit ([90a8c24](https://github.com/DaveDev42/git-worktree-manager/commit/90a8c24a8ba53f8049418dbcf52ef8c198ea8e22))
* **spawn_spec:** drop exec so launching shell survives AI exit ([05845f3](https://github.com/DaveDev42/git-worktree-manager/commit/05845f360f13340d375779ebc68b7f4df807bde8))
* **spawn_spec:** pass-2 polish and restrict integration test to Unix ([2f02adf](https://github.com/DaveDev42/git-worktree-manager/commit/2f02adfd84022a5a675edf410380c546f57db33d))
* **spawn_spec:** quote paths containing backslashes to protect Windows temp paths ([fe83a48](https://github.com/DaveDev42/git-worktree-manager/commit/fe83a48e8716ce61ff686c6a9872d70a2ac2d51e))
* suppress curl progress bar in non-terminal environments ([be49d15](https://github.com/DaveDev42/git-worktree-manager/commit/be49d152b0f94e56ba8aa48c4f959b5f7ff4466f))
* **test:** isolate external_process busy-detection test into its own binary ([#118](https://github.com/DaveDev42/git-worktree-manager/issues/118)) ([730329c](https://github.com/DaveDev42/git-worktree-manager/commit/730329c042559346913add76d0d8526c9f23f54f))
* **tests:** isolate custom_path test in dedicated TempDir ([99d329f](https://github.com/DaveDev42/git-worktree-manager/commit/99d329f0cdda74b227317570f2c28efb9e03ca7b))
* **tests:** isolate setup_remote in dedicated TempDir to prevent path collisions ([7c1e088](https://github.com/DaveDev42/git-worktree-manager/commit/7c1e088f615d3d8b4f89dcb7db025a8433a4dce8))
* **tests:** isolate test helpers and re-enable ignored tests ([183c828](https://github.com/DaveDev42/git-worktree-manager/commit/183c828c097a2e4de0666462e53c9da1114d4a88))
* **tests:** only ignore delete-cwd test on Windows via cfg_attr ([87a7f4c](https://github.com/DaveDev42/git-worktree-manager/commit/87a7f4c6fcc0d36c69a51f6ed384936c1f6ec5f9))
* **tests:** un-ignore all remote worktree tests ([6f71c54](https://github.com/DaveDev42/git-worktree-manager/commit/6f71c547baefb791fce26f460269ce324d8b8517))
* **tui:** drop let _ on unit-typed ratatui::restore ([dbc88e0](https://github.com/DaveDev42/git-worktree-manager/commit/dbc88e04b0ef52a004f2865c0751119aef72f6a6))
* upgrade companion binary from downloaded file, not installed one ([620d8f0](https://github.com/DaveDev42/git-worktree-manager/commit/620d8f0223f3869f90c16597ff4c0bc7bf8578a4))
* upgrade companion binary from downloaded file, not installed one ([7f28260](https://github.com/DaveDev42/git-worktree-manager/commit/7f28260d5273896c83d977ed83ef5639402e7cf7))
* use &gt;= in clean --older-than so 0 matches all worktrees ([a8cd381](https://github.com/DaveDev42/git-worktree-manager/commit/a8cd381a03b078771876c947ee1a09698a1b33a8))
* use &gt;= in clean --older-than so 0 matches all worktrees ([d9417d3](https://github.com/DaveDev42/git-worktree-manager/commit/d9417d3ea0d60fd378000d32def9d76bc623a2b4))
* use cyan instead of blue for pr-open status in stats bar ([d11b81d](https://github.com/DaveDev42/git-worktree-manager/commit/d11b81dfc11ae23b485085c894bbc79cd0d13d7b))
* use gh auth token for GitHub API to avoid rate limits ([ef4e498](https://github.com/DaveDev42/git-worktree-manager/commit/ef4e4987b185ea79b1df14a281f16f2d7fd3da35))
* **wezterm:** make w-t and w-t-b launchers work on Windows ([a6a6234](https://github.com/DaveDev42/git-worktree-manager/commit/a6a6234ced6f6a79f74f73ca354956f9f36f3e74))
* **wezterm:** make w-t and w-t-b launchers work on Windows ([74995dd](https://github.com/DaveDev42/git-worktree-manager/commit/74995ddf35f140b6f6573d39cf4f2181624c6f9a))
* Windows CI — UNC path comparison + cargo fmt for tests ([2a3495e](https://github.com/DaveDev42/git-worktree-manager/commit/2a3495e350623bc0c6070beaeea903bf5effae7b))
* **windows:** run main on 8 MiB stack thread to avoid clap parser overflow ([6c9613e](https://github.com/DaveDev42/git-worktree-manager/commit/6c9613ec4aa3f79d4b17e6d3850730f8c295056d))
* wire dead --bg / --fg / backup --output flags ([#111](https://github.com/DaveDev42/git-worktree-manager/issues/111)) ([61c46f8](https://github.com/DaveDev42/git-worktree-manager/commit/61c46f8832adc43d74bbfb4bad4dffee3aafa9a5))


### Performance Improvements

* **entrypoint:** skip startup scans for Path/ShellFunction completion helpers ([3c77661](https://github.com/DaveDev42/git-worktree-manager/commit/3c776610e93d6edcc6470143aafacf8726fce41f))
* **list:** batched gh, on-disk cache, parallelism, progressive ratatui rendering ([ae7b0f2](https://github.com/DaveDev42/git-worktree-manager/commit/ae7b0f27ec009e1c7fdfc154f5a4eddd78af4635))
* **list:** parallelize worktree status with rayon ([80e8e9c](https://github.com/DaveDev42/git-worktree-manager/commit/80e8e9c60a5fc77db23b8b2e7c2af9ce010373aa))
* use lockfile-only busy check in gw list ([7275a80](https://github.com/DaveDev42/git-worktree-manager/commit/7275a800b82c5a6f5c121fda7f80b4bacf80cb18))
* use lockfile-only busy check in gw list ([3f76ca2](https://github.com/DaveDev42/git-worktree-manager/commit/3f76ca22a6481cf260234e6780cb906897a42fda))
* zero-latency startup update check with background refresh ([cb6550d](https://github.com/DaveDev42/git-worktree-manager/commit/cb6550d296bb9beebb5ffa58674629c0efbb108b))

## [0.0.42](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.41...v0.0.42) (2026-04-30)


### Features

* **spawn-ai:** enable no-arg recovery from corrupted launcher line ([#125](https://github.com/DaveDev42/git-worktree-manager/issues/125)) ([48e3c4f](https://github.com/DaveDev42/git-worktree-manager/commit/48e3c4fbbaf2831f462d444345c933f007440a44))
* **tui:** show relative age and busy badge in delete -i selector ([#116](https://github.com/DaveDev42/git-worktree-manager/issues/116)) ([dea9e43](https://github.com/DaveDev42/git-worktree-manager/commit/dea9e43ca5271e2c06d11a65f27c820675126f78))


### Bug Fixes

* **test:** isolate external_process busy-detection test into its own binary ([#118](https://github.com/DaveDev42/git-worktree-manager/issues/118)) ([730329c](https://github.com/DaveDev42/git-worktree-manager/commit/730329c042559346913add76d0d8526c9f23f54f))

## [0.0.41](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.40...v0.0.41) (2026-04-28)


### Bug Fixes

* **busy:** require live claude process to flag worktree busy ([#112](https://github.com/DaveDev42/git-worktree-manager/issues/112)) ([49a622a](https://github.com/DaveDev42/git-worktree-manager/commit/49a622a33ccdd2f328f73d4ee4230d6a31146c09))
* wire dead --bg / --fg / backup --output flags ([#111](https://github.com/DaveDev42/git-worktree-manager/issues/111)) ([61c46f8](https://github.com/DaveDev42/git-worktree-manager/commit/61c46f8832adc43d74bbfb4bad4dffee3aafa9a5))

## [0.0.40](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.39...v0.0.40) (2026-04-28)


### Bug Fixes

* **clean:** use PrCache for --merged detection to handle squash merges ([#109](https://github.com/DaveDev42/git-worktree-manager/issues/109)) ([ee80a26](https://github.com/DaveDev42/git-worktree-manager/commit/ee80a26d4507714340142f45955d9d0536bf0806))
* **setup-claude:** install plugin via local Claude Code marketplace ([#107](https://github.com/DaveDev42/git-worktree-manager/issues/107)) ([2c9370e](https://github.com/DaveDev42/git-worktree-manager/commit/2c9370e47d4c8c5c8d591b79727e8249b1413e41))

## [0.0.39](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.38...v0.0.39) (2026-04-25)


### Features

* **upgrade:** add --yes flag for non-interactive upgrade ([#104](https://github.com/DaveDev42/git-worktree-manager/issues/104)) ([5ada374](https://github.com/DaveDev42/git-worktree-manager/commit/5ada374156ddb7a859848a004ae32b3aa21c6c20))

## [0.0.38](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.37...v0.0.38) (2026-04-25)


### Features

* **busy:** detect_busy_tiered combines Claude session + lockfile (Hard) and scan (Soft) ([45d5f9c](https://github.com/DaveDev42/git-worktree-manager/commit/45d5f9c40dd3520ffafb465b4a779171e59662cb))
* **busy:** refusal message renderer for tiered busy model ([3d717db](https://github.com/DaveDev42/git-worktree-manager/commit/3d717db227054878a045611aa96980f3053970a9))
* **claude_session:** find_active_sessions with threshold + cwd filter ([aef7172](https://github.com/DaveDev42/git-worktree-manager/commit/aef7172d4d9848ac45c0759f789dff7a16a12f0b))
* **claude_session:** newest_event_timestamp / cwd jsonl tail parsers ([7b74c8e](https://github.com/DaveDev42/git-worktree-manager/commit/7b74c8e4379e7885cba812e9af4e047fe1fcd825))
* **claude_session:** path encoding for ~/.claude/projects/ lookup ([dbcc655](https://github.com/DaveDev42/git-worktree-manager/commit/dbcc655ed7363a4203ed8b31a9042884955622ef))
* **cli:** accept multiple delete targets and -i/--dry-run flags ([320a472](https://github.com/DaveDev42/git-worktree-manager/commit/320a472ffbef195c49b1bcb50831413c64640ce7))
* **delete_batch:** add plan types and resolver ([ef2e834](https://github.com/DaveDev42/git-worktree-manager/commit/ef2e8343efdd9177946dcf6de83abb266d431d85))
* **delete_batch:** add summary printer and batch confirmation ([908ca1d](https://github.com/DaveDev42/git-worktree-manager/commit/908ca1dc28dde40ef2362029de0c1a83bd313489))
* **delete:** multi-target support with -i and --dry-run ([6713359](https://github.com/DaveDev42/git-worktree-manager/commit/6713359e4aea19de098e1b7638e093d63a54c53e))
* **delete:** orchestrate multi-target delete with dry-run and batch confirm ([d186c5a](https://github.com/DaveDev42/git-worktree-manager/commit/d186c5a87fe4e70ac68521d8a46d089a22abd01d))
* **delete:** tiered refusal in batch + clean paths ([a247dd8](https://github.com/DaveDev42/git-worktree-manager/commit/a247dd8b6994df406ebb1f2ff35706eb28356472))
* **delete:** tiered refusal messages, drop interactive y/N prompt ([e3f41ba](https://github.com/DaveDev42/git-worktree-manager/commit/e3f41ba2a5a9340506cf6bfdf67175edcc7b09e0))
* **doctor:** --session-start --quiet single-line hook-friendly mode ([0deecda](https://github.com/DaveDev42/git-worktree-manager/commit/0deecda364131e5cd4deff2a45cfac563e2f9f83))
* **doctor:** detect plugin install + suggest upgrade from legacy skill ([2f031e9](https://github.com/DaveDev42/git-worktree-manager/commit/2f031e9efd3672cbe1d09a4360943181aa0bbcce))
* **guard:** hook helper to block risky bash in unhealthy cwd ([1d17d58](https://github.com/DaveDev42/git-worktree-manager/commit/1d17d584140a09e7268ead58359bbb49d6c9b051))
* gw plugin + worktree-health skill + tiered in-use detection ([513645d](https://github.com/DaveDev42/git-worktree-manager/commit/513645d0b0d5e9b859eac70ce681fbcb3c332b44))
* **setup_claude:** manage skill body — guidance + rulebook + hook catalog ([89b7490](https://github.com/DaveDev42/git-worktree-manager/commit/89b7490989d17bc1f42a6ebb979884978f8b3dfa))
* **setup_claude:** migrate existing skill body into delegate skill ([94bfe0f](https://github.com/DaveDev42/git-worktree-manager/commit/94bfe0fb5fe64cd1425225fc010e9ec04537082b))
* **tui:** add multi-select widget and wire gw delete -i ([b4b37f1](https://github.com/DaveDev42/git-worktree-manager/commit/b4b37f1585046c251ef7de6d03698dca116730da))


### Bug Fixes

* **delete_batch:** distinguish Nothing-selected from Cancelled in -i flow ([de39b00](https://github.com/DaveDev42/git-worktree-manager/commit/de39b009699efc93d7ecb08b304842f8b289687a))
* **delete_batch:** route summary to stdout and add dry-run trailer ([ca85db6](https://github.com/DaveDev42/git-worktree-manager/commit/ca85db6f752ffbbae7421dcd75e32e72cd92a6ba))
* **deps:** bump ratatui 0.28.1 → 0.30.0 and migrate TUI code ([c1d3035](https://github.com/DaveDev42/git-worktree-manager/commit/c1d3035aa1766002a84c26375746cf5e8a1a02fa))
* **deps:** bump ratatui to 0.30 ([befe0e7](https://github.com/DaveDev42/git-worktree-manager/commit/befe0e785e476966753e045b0ec26223b061c066))
* **setup_claude:** add setup_claude_under(home) so tests can install hermetically on Windows ([44f1bc1](https://github.com/DaveDev42/git-worktree-manager/commit/44f1bc1a261577fbbc41f2f62fcf3eec5ec1e747))
* **windows:** run main on 8 MiB stack thread to avoid clap parser overflow ([6c9613e](https://github.com/DaveDev42/git-worktree-manager/commit/6c9613ec4aa3f79d4b17e6d3850730f8c295056d))

## [0.0.37](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.36...v0.0.37) (2026-04-23)


### Bug Fixes

* **spawn_spec:** drop exec so launching shell survives AI exit ([90a8c24](https://github.com/DaveDev42/git-worktree-manager/commit/90a8c24a8ba53f8049418dbcf52ef8c198ea8e22))
* **spawn_spec:** drop exec so launching shell survives AI exit ([05845f3](https://github.com/DaveDev42/git-worktree-manager/commit/05845f360f13340d375779ebc68b7f4df807bde8))

## [0.0.36](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.35...v0.0.36) (2026-04-22)


### Features

* **spawn_spec:** add 24h sweep_stale for crash-residue cleanup ([4516521](https://github.com/DaveDev42/git-worktree-manager/commit/451652144661d04b3916f3dd32903281e9be3637))
* **spawn_spec:** implement execute reading spec and execvp-ing target ([865f766](https://github.com/DaveDev42/git-worktree-manager/commit/865f76604fae8f7aedea23ea72b4d89a639b6240))
* **spawn_spec:** implement materialize writing 0600 tempfile ([6166cbb](https://github.com/DaveDev42/git-worktree-manager/commit/6166cbb712fe1f60c481ee199b10df11444bd0aa))
* **spawn_spec:** scaffold SpawnSpec struct with round-trip test ([be10b65](https://github.com/DaveDev42/git-worktree-manager/commit/be10b659f05b512d1ad9f87b08fd4fe5e36c185c))


### Bug Fixes

* **ai-tools:** eliminate shell-escape failures via gw _spawn-ai wrapper ([567e129](https://github.com/DaveDev42/git-worktree-manager/commit/567e129ce678ed8c02cf6e61c95cbe3f9edbcfce))
* **ai-tools:** route AI spawn through gw _spawn-ai wrapper ([b6ca8a2](https://github.com/DaveDev42/git-worktree-manager/commit/b6ca8a2f0e1845ca8a6542c7d5726a7fa29a252a))
* **spawn_spec:** address review feedback (exit 127, TOCTOU, spec alignment) ([9faa657](https://github.com/DaveDev42/git-worktree-manager/commit/9faa65790be7369c61eb561df9439e2a80f2ced6))
* **spawn_spec:** pass-2 polish and restrict integration test to Unix ([2f02adf](https://github.com/DaveDev42/git-worktree-manager/commit/2f02adfd84022a5a675edf410380c546f57db33d))
* **spawn_spec:** quote paths containing backslashes to protect Windows temp paths ([fe83a48](https://github.com/DaveDev42/git-worktree-manager/commit/fe83a48e8716ce61ff686c6a9872d70a2ac2d51e))


### Performance Improvements

* **entrypoint:** skip startup scans for Path/ShellFunction completion helpers ([3c77661](https://github.com/DaveDev42/git-worktree-manager/commit/3c776610e93d6edcc6470143aafacf8726fce41f))

## [0.0.35](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.34...v0.0.35) (2026-04-21)


### Bug Fixes

* **wezterm:** make w-t and w-t-b launchers work on Windows ([a6a6234](https://github.com/DaveDev42/git-worktree-manager/commit/a6a6234ced6f6a79f74f73ca354956f9f36f3e74))
* **wezterm:** make w-t and w-t-b launchers work on Windows ([74995dd](https://github.com/DaveDev42/git-worktree-manager/commit/74995ddf35f140b6f6573d39cf4f2181624c6f9a))

## [0.0.34](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.33...v0.0.34) (2026-04-21)


### Bug Fixes

* **backup:** show main-worktree backups and sanitize slash-in-branch ([9ea6c6e](https://github.com/DaveDev42/git-worktree-manager/commit/9ea6c6ea2885c5e8850ea72adf8fceee61d84fe5))

## [0.0.33](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.32...v0.0.33) (2026-04-14)


### Performance Improvements

* use lockfile-only busy check in gw list ([7275a80](https://github.com/DaveDev42/git-worktree-manager/commit/7275a800b82c5a6f5c121fda7f80b4bacf80cb18))
* use lockfile-only busy check in gw list ([3f76ca2](https://github.com/DaveDev42/git-worktree-manager/commit/3f76ca22a6481cf260234e6780cb906897a42fda))

## [0.0.32](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.31...v0.0.32) (2026-04-14)


### Features

* **cli:** add --prompt-file and --prompt-stdin to gw new ([30f8086](https://github.com/DaveDev42/git-worktree-manager/commit/30f80861c1fc53349d470d220cba0194be2a8722))
* **cli:** add --prompt-file and --prompt-stdin to gw new ([0425c7a](https://github.com/DaveDev42/git-worktree-manager/commit/0425c7a25f567ed10fb6a348bb4287d5dda45423))
* **cli:** resolve prompt from --prompt/--prompt-file/--prompt-stdin ([235fac2](https://github.com/DaveDev42/git-worktree-manager/commit/235fac24e0130410fe0340d268256631fe5dd548))

## [0.0.31](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.30...v0.0.31) (2026-04-13)


### Bug Fixes

* **busy:** clean up process name and pgid-based sibling exclusion ([92d6d5b](https://github.com/DaveDev42/git-worktree-manager/commit/92d6d5ba87a6eeb302c92ab2904e4483d6f3817b))
* **busy:** resolve argv-rewritten cmd and exclude pipeline siblings ([6d2e957](https://github.com/DaveDev42/git-worktree-manager/commit/6d2e957a6537c645f30473829a6c86561a9263ed))

## [0.0.30](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.29...v0.0.30) (2026-04-13)


### Features

* **cli:** --no-cache on clean and global; parallelize TODO; rename pr_state ([f324ae6](https://github.com/DaveDev42/git-worktree-manager/commit/f324ae600c2cb762e8bfe469caca5357091a6a54))
* **cli:** add --no-cache flag to list subcommand ([4b96b17](https://github.com/DaveDev42/git-worktree-manager/commit/4b96b1731017cc4ce6c6a8a1f68bc1a283d2286e))
* **list:** progressive Inline Viewport rendering in TTY mode ([67bae1d](https://github.com/DaveDev42/git-worktree-manager/commit/67bae1d7f5f2232c3429623eb768c9b37f6ba08f))
* **list:** restore terminal on err, early-return on empty worktree set ([268758f](https://github.com/DaveDev42/git-worktree-manager/commit/268758f996ebc2084fa67a15a3fe5f0d83aafee0))
* **pr_cache:** add module skeleton with repo-hash and cache path ([85dd3b7](https://github.com/DaveDev42/git-worktree-manager/commit/85dd3b7b69b7d1985c28a1de0ac3aa559a3fdc3a))
* **pr_cache:** fetch PR state from gh in one batched call ([3bfbe53](https://github.com/DaveDev42/git-worktree-manager/commit/3bfbe53e40b35151f1b6d959452aec950087cd1d))
* **pr_cache:** persist PR state to disk with 60s TTL ([2c7699b](https://github.com/DaveDev42/git-worktree-manager/commit/2c7699ba3f0cee9f3a7b9ecd84bf2e7ce8d1309e))
* **pr_cache:** public load_or_fetch with --no-cache semantics ([f61b3dd](https://github.com/DaveDev42/git-worktree-manager/commit/f61b3dd9265df6781ea6ab977fc175d253357ebf))
* **tui:** Inline Viewport app skeleton with snapshot tests ([8084246](https://github.com/DaveDev42/git-worktree-manager/commit/8084246b7cdafedc9870bc02c1823842501a234f))
* **tui:** progressive render loop consuming mpsc updates ([6f4f52a](https://github.com/DaveDev42/git-worktree-manager/commit/6f4f52a25772773da9dae09be8bde87172a1a05e))
* **tui:** promote tui to directory module, add style palette and panic hook ([c5e6855](https://github.com/DaveDev42/git-worktree-manager/commit/c5e685529884ca3f0e930827cc2600421b9a85f0))


### Bug Fixes

* **list:** apply all 52 review items — correctness, hygiene, API, polish, docs ([3d7a942](https://github.com/DaveDev42/git-worktree-manager/commit/3d7a942c04a59c18e835273c26ea03d7c986d8f1))
* **list:** correctness, behavior polish, docs, tests — apply pass-2 items A/B/D/E ([ec2e0a9](https://github.com/DaveDev42/git-worktree-manager/commit/ec2e0a95f8a51c9a522e6598e25b68b06b0affde))
* **list:** panic-safe sweep, in_place_scope, extract footer helper ([d4b64d2](https://github.com/DaveDev42/git-worktree-manager/commit/d4b64d2ec9152d6bb15f852ec72f4b1a43daf1aa))
* **list:** pass-3 correctness — atomic flag ordering, panic readability, ratatui setup race ([0d074e8](https://github.com/DaveDev42/git-worktree-manager/commit/0d074e8a094d388277a1abef93cfa8319eab9bb4))
* **list:** pass-4 — env safety, prefetch filter, exhaustive PrState, comment fixes ([1e91d72](https://github.com/DaveDev42/git-worktree-manager/commit/1e91d727cb9b09063ee772f0fe78695c0a7b7451))
* **list:** pass-5 final — sweep scope, panic message clarity, test rigor ([89d472e](https://github.com/DaveDev42/git-worktree-manager/commit/89d472ec6f4b043d14390be7d7563551ba696bb3))
* **tui:** drop let _ on unit-typed ratatui::restore ([dbc88e0](https://github.com/DaveDev42/git-worktree-manager/commit/dbc88e04b0ef52a004f2865c0751119aef72f6a6))


### Performance Improvements

* **list:** batched gh, on-disk cache, parallelism, progressive ratatui rendering ([ae7b0f2](https://github.com/DaveDev42/git-worktree-manager/commit/ae7b0f27ec009e1c7fdfc154f5a4eddd78af4635))
* **list:** parallelize worktree status with rayon ([80e8e9c](https://github.com/DaveDev42/git-worktree-manager/commit/80e8e9c60a5fc77db23b8b2e7c2af9ce010373aa))

## [0.0.29](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.28...v0.0.29) (2026-04-13)


### Bug Fixes

* **busy:** broaden multiplexer match + disable lsof truncation ([22f7778](https://github.com/DaveDev42/git-worktree-manager/commit/22f777855cfc3af80e258495f7500281a08670be))
* **busy:** exclude terminal multiplexer servers from cwd scan ([5711658](https://github.com/DaveDev42/git-worktree-manager/commit/571165893a872731767827330b0ab6d95d13808a))

## [0.0.28](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.27...v0.0.28) (2026-04-12)


### Features

* acquire session lockfile when entering gw shell ([24da17e](https://github.com/DaveDev42/git-worktree-manager/commit/24da17e2fbe15d93e982b3cd1b98f72f3e4466ef))
* acquire session lockfile when launching AI tools ([a0a2a34](https://github.com/DaveDev42/git-worktree-manager/commit/a0a2a34074f8a5aa9049fb46e21249bda179a5a3))
* add busy detection with lockfile + process cwd scan ([102435d](https://github.com/DaveDev42/git-worktree-manager/commit/102435dc4334c868f6f38b09ac07026b04bf1abd))
* add busy status to worktree state with highest non-stale priority ([ae18947](https://github.com/DaveDev42/git-worktree-manager/commit/ae189470d1d74a5cc22ab1e1eeac9da96411dda8))
* add session lockfile with RAII guard and PID liveness check ([2fc68aa](https://github.com/DaveDev42/git-worktree-manager/commit/2fc68aa83f922afc7beea205e4ead609e2e6b35f))
* **clean:** skip busy worktrees and report summary, --force to override ([1fc20ee](https://github.com/DaveDev42/git-worktree-manager/commit/1fc20ee3fcabb7aef910ee4831b75e098a3f373e))
* **delete:** block busy worktree deletion with TTY-aware prompt ([9d33e08](https://github.com/DaveDev42/git-worktree-manager/commit/9d33e08a46a62cbbd918487d2e4dc238c40b51fa))
* **display,tests:** show busy in tree/stats UI; robust busy integration tests ([a9b69b0](https://github.com/DaveDev42/git-worktree-manager/commit/a9b69b099acef86aee0d064901f34121ce09c1e0))
* hybrid worktree busy detection for delete/clean ([1abcb24](https://github.com/DaveDev42/git-worktree-manager/commit/1abcb245dc7f3620c42c63d315c09ecc12bc7bb6))


### Bug Fixes

* **busy-detection:** address review pass 2 issues ([4e2e9bb](https://github.com/DaveDev42/git-worktree-manager/commit/4e2e9bb7b171db132e66aa67c7384cc6941e14a6))
* **busy:** local Command import in scan_cwd test so Linux compiles ([feb3472](https://github.com/DaveDev42/git-worktree-manager/commit/feb347278a2bf5cbae0d7cd2cc52c89067f2c217))
* **busy:** narrow Command import to macos; Linux uses /proc ([4687770](https://github.com/DaveDev42/git-worktree-manager/commit/4687770a43dbea42a5605d2fe2d8306674100d75))
* **busy:** verify cwd is inside target on macOS lsof parse ([cfe0346](https://github.com/DaveDev42/git-worktree-manager/commit/cfe03464e7cebff5c7d89a63cdbd33cb47483351))
* critical busy-detection bugs found during smoke testing ([6fe35d8](https://github.com/DaveDev42/git-worktree-manager/commit/6fe35d83703ee981f4383ddec29c6800c7b19c0f))
* **delete:** use explicit --force as busy override, not default git-force flag ([02a8a4e](https://github.com/DaveDev42/git-worktree-manager/commit/02a8a4ecadc459fc6af70d9c1d31e0d7074232eb))
* **lockfile:** cfg-gate STALE_TTL so unix build does not warn ([44469dd](https://github.com/DaveDev42/git-worktree-manager/commit/44469ddc74eef095d35f0264c72de194fbbb0fea))
* **lockfile:** post-rename ownership check, thiserror, tmp cleanup sweep, version field ([41d1ef9](https://github.com/DaveDev42/git-worktree-manager/commit/41d1ef99036bca39aea4dbebded894e049b59002))

## [0.0.27](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.26...v0.0.27) (2026-04-08)


### Bug Fixes

* auto-update Homebrew formula on release ([517eefa](https://github.com/DaveDev42/git-worktree-manager/commit/517eefac6081e57780767a8c0add520e66dc79db))
* auto-update Homebrew formula on release ([b40b091](https://github.com/DaveDev42/git-worktree-manager/commit/b40b09117bf4d6fdde8938951b8dd83a1bfceaa5))

## [0.0.26](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.25...v0.0.26) (2026-04-06)


### Bug Fixes

* add --fail to version check curl and validate curl in PATH ([b062de8](https://github.com/DaveDev42/git-worktree-manager/commit/b062de8ff4a95e77c88263349df9b8e300a51da5))
* add curl PATH check to fetch_latest_version for consistency ([88c50a6](https://github.com/DaveDev42/git-worktree-manager/commit/88c50a6672057c4b61627bef3d7fcb6a7837ccd4))
* improve curl error message and add User-Agent to version check ([6b1bd42](https://github.com/DaveDev42/git-worktree-manager/commit/6b1bd425366fdf0f506188220440f1538147ff1a))
* suppress curl progress bar in non-terminal environments ([be49d15](https://github.com/DaveDev42/git-worktree-manager/commit/be49d152b0f94e56ba8aa48c4f959b5f7ff4466f))

## [0.0.25](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.24...v0.0.25) (2026-04-06)


### Bug Fixes

* extract testable pure function and add unit tests for tab lookup ([a9f9335](https://github.com/DaveDev42/git-worktree-manager/commit/a9f93358a03aa70f34576038dc7278e798b38b70))
* restore focus to active tab instead of calling tab in w-t-b launcher ([3354dc1](https://github.com/DaveDev42/git-worktree-manager/commit/3354dc1dc291e154996a7986b9ec6aa935d2daea))
* restore focus to active tab instead of calling tab in w-t-b launcher ([f785abd](https://github.com/DaveDev42/git-worktree-manager/commit/f785abd5c7b87e477058f70def166288878cccb6))

## [0.0.24](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.23...v0.0.24) (2026-04-05)


### Features

* add get_pr_state helper to query GitHub PR status via gh CLI ([d217d5f](https://github.com/DaveDev42/git-worktree-manager/commit/d217d5febbf0b5357b7adb8d9ddfe728838c3fd3))
* add icon and color mappings for merged and pr-open worktree statuses ([c917728](https://github.com/DaveDev42/git-worktree-manager/commit/c91772897ed5ac324e51a6e8287a8c97b018035a))
* add merged and pr-open worktree statuses ([3096081](https://github.com/DaveDev42/git-worktree-manager/commit/3096081667e46d52e3a39029d00fa05aaee911c2))
* add WezTerm background tab (w-t-b) + fix iTerm AppleScript ([dd69f47](https://github.com/DaveDev42/git-worktree-manager/commit/dd69f47fac6db0ba768c64a7992244a415aa3f4a))
* add WezTerm background tab launch method (w-t-b) ([cac993c](https://github.com/DaveDev42/git-worktree-manager/commit/cac993ce8af789d1825995782b4de4896223b0d8))
* integrate merged and pr-open statuses into worktree display ([ccd518e](https://github.com/DaveDev42/git-worktree-manager/commit/ccd518e15cb24d7b75fc8c3b999956de112ffff9))


### Bug Fixes

* **tests:** isolate custom_path test in dedicated TempDir ([99d329f](https://github.com/DaveDev42/git-worktree-manager/commit/99d329f0cdda74b227317570f2c28efb9e03ca7b))
* **tests:** isolate setup_remote in dedicated TempDir to prevent path collisions ([7c1e088](https://github.com/DaveDev42/git-worktree-manager/commit/7c1e088f615d3d8b4f89dcb7db025a8433a4dce8))
* **tests:** isolate test helpers and re-enable ignored tests ([183c828](https://github.com/DaveDev42/git-worktree-manager/commit/183c828c097a2e4de0666462e53c9da1114d4a88))
* **tests:** only ignore delete-cwd test on Windows via cfg_attr ([87a7f4c](https://github.com/DaveDev42/git-worktree-manager/commit/87a7f4c6fcc0d36c69a51f6ed384936c1f6ec5f9))
* **tests:** un-ignore all remote worktree tests ([6f71c54](https://github.com/DaveDev42/git-worktree-manager/commit/6f71c547baefb791fce26f460269ce324d8b8517))
* use &gt;= in clean --older-than so 0 matches all worktrees ([a8cd381](https://github.com/DaveDev42/git-worktree-manager/commit/a8cd381a03b078771876c947ee1a09698a1b33a8))
* use &gt;= in clean --older-than so 0 matches all worktrees ([d9417d3](https://github.com/DaveDev42/git-worktree-manager/commit/d9417d3ea0d60fd378000d32def9d76bc623a2b4))
* use cyan instead of blue for pr-open status in stats bar ([d11b81d](https://github.com/DaveDev42/git-worktree-manager/commit/d11b81dfc11ae23b485085c894bbc79cd0d13d7b))

## [0.0.23](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.22...v0.0.23) (2026-04-05)


### Bug Fixes

* create draft releases until binaries are uploaded ([319829c](https://github.com/DaveDev42/git-worktree-manager/commit/319829cdcd51f9a73b4e6718e850bcaab385c537))
* create GitHub releases as draft until binaries are uploaded ([4b5cb45](https://github.com/DaveDev42/git-worktree-manager/commit/4b5cb4589c8f72e990fdc55d46ecdd5cc970c800))

## [0.0.22](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.21...v0.0.22) (2026-04-05)


### Features

* rename gw-delegate skill to gw with natural language support ([30f2012](https://github.com/DaveDev42/git-worktree-manager/commit/30f20122c226c6fd7264fc4e7a86efb2eb46346c))
* rename gw-delegate skill to gw with natural language support ([dfe1fcb](https://github.com/DaveDev42/git-worktree-manager/commit/dfe1fcb0ba43cfa01726813a466040104c585595))
* suggest .claude/settings.local.json in .cwshare setup ([1e742ba](https://github.com/DaveDev42/git-worktree-manager/commit/1e742ba5cf643daf4e8f634c3f27b71b720dff9f))
* suggest .claude/settings.local.json in .cwshare setup ([4594ef6](https://github.com/DaveDev42/git-worktree-manager/commit/4594ef6ccaeaf0bf819686d3d53f24d24482cea7))

## [0.0.21](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.20...v0.0.21) (2026-04-04)


### Bug Fixes

* upgrade companion binary from downloaded file, not installed one ([620d8f0](https://github.com/DaveDev42/git-worktree-manager/commit/620d8f0223f3869f90c16597ff4c0bc7bf8578a4))
* upgrade companion binary from downloaded file, not installed one ([7f28260](https://github.com/DaveDev42/git-worktree-manager/commit/7f28260d5273896c83d977ed83ef5639402e7cf7))

## [0.0.20](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.19...v0.0.20) (2026-04-04)


### Bug Fixes

* companion binary overwritten with old version during upgrade ([c6d4334](https://github.com/DaveDev42/git-worktree-manager/commit/c6d43341c1e1ae16f7bec10783c56d81578601d2))
* companion binary overwritten with old version during upgrade ([505f4d9](https://github.com/DaveDev42/git-worktree-manager/commit/505f4d9dc3c38662ea3df2b038ee09fc4aebebfd))

## [0.0.19](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.18...v0.0.19) (2026-04-04)


### Features

* add Claude Code skill for worktree task delegation ([52f0d06](https://github.com/DaveDev42/git-worktree-manager/commit/52f0d06d24fb258780c1c7e1549cfacda50cdf94))
* add Claude Code skill for worktree task delegation ([478619b](https://github.com/DaveDev42/git-worktree-manager/commit/478619bf4d95389f473feaed594a3a8ef407ae5a))
* add dynamic branch name tab completion for all subcommands ([b2d895c](https://github.com/DaveDev42/git-worktree-manager/commit/b2d895cd3c9cf91e40a98d271964b73bc567dc39))
* add SHA256 checksums to release artifacts ([d70336e](https://github.com/DaveDev42/git-worktree-manager/commit/d70336ef9aada8ec58e0c66f53ed873342306bea))
* auto-detect repository default branch ([2e4321c](https://github.com/DaveDev42/git-worktree-manager/commit/2e4321c62c5d4a1274eece01b901c599fcf16a8b))
* auto-upgrade via self-replace from GitHub Releases ([48b948e](https://github.com/DaveDev42/git-worktree-manager/commit/48b948e7a61ebb2826f7006ea19ee5f47b2beb76))
* centralize messages and add format_age tests ([b675c83](https://github.com/DaveDev42/git-worktree-manager/commit/b675c83e085dabfc2548c669d7856c18714870d6))
* code quality — extract helpers, split doctor, shell completion prompt ([faa6eba](https://github.com/DaveDev42/git-worktree-manager/commit/faa6ebad0c4984a24bda9bf8ed79055c4a4f17df))
* code quality refactoring, shell completion parity, CI improvements ([e00938c](https://github.com/DaveDev42/git-worktree-manager/commit/e00938cadad1d214c71c0580769dc1ac7a206293))
* Complete feature parity — all missing commands and options ([908b81c](https://github.com/DaveDev42/git-worktree-manager/commit/908b81c461db3195d3f9d2db949a5d4251c1d169))
* Complete Python feature parity — all 15 items implemented ([e4d86ba](https://github.com/DaveDev42/git-worktree-manager/commit/e4d86babb8b58017db342f3145fe1d06a5eac57b))
* config list table, config key tab completion, cw bash completion ([8ad734f](https://github.com/DaveDev42/git-worktree-manager/commit/8ad734f14dd736d693532f1c47b42a98fcf3f2c2))
* expand skill with full gw reference and references/ support ([aa1a9c1](https://github.com/DaveDev42/git-worktree-manager/commit/aa1a9c123b89b5f3000af3ddaba0434bab2a4b39))
* improve tab completion for all option values ([c5bb0d0](https://github.com/DaveDev42/git-worktree-manager/commit/c5bb0d08ed1e66b0d54d1086ed9731cf286c2194))
* improve tab completion for all option values ([08dc42a](https://github.com/DaveDev42/git-worktree-manager/commit/08dc42a8dafdc1e527a2fb466067f6f59785c527))
* Phase 1 — project scaffolding with core infrastructure ([8bdeb88](https://github.com/DaveDev42/git-worktree-manager/commit/8bdeb887aba1b69c6627ed4d67a9a1e927142eb6))
* Phase 2 — core commands (new, delete, merge, pr, sync, config) ([c310fe6](https://github.com/DaveDev42/git-worktree-manager/commit/c310fe65fc0c8bc198fa4a06f2740517d9021369))
* Phase 3 — terminal launchers and AI tool integration ([5e9d07b](https://github.com/DaveDev42/git-worktree-manager/commit/5e9d07bb3120732bc00d4d78f457f025237db59f))
* Phase 4 — shell functions, auto-update, and upgrade ([e99b7c7](https://github.com/DaveDev42/git-worktree-manager/commit/e99b7c7aeb2424fdf4b58964e06553c00593a817))
* Phase 5 — backup, diagnostics, CI/CD, and polish ([983bdc3](https://github.com/DaveDev42/git-worktree-manager/commit/983bdc37ed8a73fef02bc61ffac76ea8ef637bd7))
* Python parity improvements — CLI flags, config fallback, colored hooks ([a3c8ee2](https://github.com/DaveDev42/git-worktree-manager/commit/a3c8ee2021bd7102a1db8ba8e4f35c875e611726))
* QA improvements — config get, delete --force, hook aliases, backup filtering ([b8a7cc0](https://github.com/DaveDev42/git-worktree-manager/commit/b8a7cc048b4593f935646d21245cccd1cd09dc67))
* refresh shell cache on gw shell-setup ([eebde54](https://github.com/DaveDev42/git-worktree-manager/commit/eebde549cb89446451aa261fe29d0ad45680c504))
* register gw/cw tab completion in shell functions ([1eab90d](https://github.com/DaveDev42/git-worktree-manager/commit/1eab90d615edb5d5803a4fcc854968f1b7bcd740))
* Rename to git-worktree-manager (gw) ([3c061f1](https://github.com/DaveDev42/git-worktree-manager/commit/3c061f1d426eda942664f770ef81fa1a0e4d0aaa))
* rich TUI output, duration parsing, hook error formatting ([7047b87](https://github.com/DaveDev42/git-worktree-manager/commit/7047b873c1175315156613b61d9ff42da27b9c9b))
* show launch method display names in config show/list ([b0f552f](https://github.com/DaveDev42/git-worktree-manager/commit/b0f552f9ed35a93ae2658fb77ea23f6183b97cdb))
* smart config set for ai_tool and launch.method ([79f2102](https://github.com/DaveDev42/git-worktree-manager/commit/79f21026a5a742fe92c739543848e03b5a2c63e7))
* use self_update crate for upgrade, detect Homebrew installs ([eb6aff4](https://github.com/DaveDev42/git-worktree-manager/commit/eb6aff48025f55d03a78213ac861cfe2924cc824))


### Bug Fixes

* Add cw-cd backward compatibility alias ([#9](https://github.com/DaveDev42/git-worktree-manager/issues/9)) ([f2a1361](https://github.com/DaveDev42/git-worktree-manager/commit/f2a13615848dc8883e1cad56351b4e337ab30b13))
* bump MSRV to 1.85, fix rustfmt formatting ([e59ff8e](https://github.com/DaveDev42/git-worktree-manager/commit/e59ff8ec11f28d88175f4b5867d09377c21a13eb))
* check GITHUB_TOKEN/GH_TOKEN env before spawning gh CLI ([e62c3f7](https://github.com/DaveDev42/git-worktree-manager/commit/e62c3f78d87bce74fe7e55f9d756514fd092b9be))
* disable native-tls in self_update to fix cross-compilation ([cf359f5](https://github.com/DaveDev42/git-worktree-manager/commit/cf359f5cf7220775b398f557f9ce72d27e500b5d))
* disable zip default-features to fix MSRV 1.85 and remove time vulnerability ([78ffb28](https://github.com/DaveDev42/git-worktree-manager/commit/78ffb283c609c566d2d50d71adb101ce1db261de))
* pin time and base64ct for MSRV 1.85 compatibility ([edf490d](https://github.com/DaveDev42/git-worktree-manager/commit/edf490dbbe8e4e72addb949b79e64234de786c21))
* prevent env var race condition and config leakage in launch method tests ([378c4eb](https://github.com/DaveDev42/git-worktree-manager/commit/378c4ebd5435d118b4f7b3471b81fc0a49b86fdf))
* prevent env var race condition in launch method tests ([cf0e62f](https://github.com/DaveDev42/git-worktree-manager/commit/cf0e62fef81c03f50dd8f5986f89b095db63de9c))
* remove invalid [warnings] section from audit.toml ([67433d3](https://github.com/DaveDev42/git-worktree-manager/commit/67433d316de723256e3fc503cf3516a6dd93d91c))
* replace self_update with reqwest[rustls-native-roots] for enterprise MDM TLS support ([cb12a30](https://github.com/DaveDev42/git-worktree-manager/commit/cb12a30aff4315857460e577572e0a8fcc2dbd81))
* restore audit.toml ignores for zip→time and indicatif→number_prefix ([ec6ceed](https://github.com/DaveDev42/git-worktree-manager/commit/ec6ceedf0c71d5e8644af8cc1a10ebd1cbedc39a))
* revert version to 0.0.1 so Release Please creates v0.0.2 ([addd37f](https://github.com/DaveDev42/git-worktree-manager/commit/addd37f57d33d8bab48fe5583411dfdc2df1aa96))
* skip Unix-only tests on Windows ([79ce973](https://github.com/DaveDev42/git-worktree-manager/commit/79ce973f96902d9351ebea05d3edbfb6d6bda24a))
* use gh auth token for GitHub API to avoid rate limits ([ef4e498](https://github.com/DaveDev42/git-worktree-manager/commit/ef4e4987b185ea79b1df14a281f16f2d7fd3da35))
* Windows CI — UNC path comparison + cargo fmt for tests ([2a3495e](https://github.com/DaveDev42/git-worktree-manager/commit/2a3495e350623bc0c6070beaeea903bf5effae7b))


### Performance Improvements

* zero-latency startup update check with background refresh ([cb6550d](https://github.com/DaveDev42/git-worktree-manager/commit/cb6550d296bb9beebb5ffa58674629c0efbb108b))

## [0.0.18](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.17...v0.0.18) (2026-04-04)


### Features

* improve tab completion for all option values ([c5bb0d0](https://github.com/DaveDev42/git-worktree-manager/commit/c5bb0d08ed1e66b0d54d1086ed9731cf286c2194))
* improve tab completion for all option values ([08dc42a](https://github.com/DaveDev42/git-worktree-manager/commit/08dc42a8dafdc1e527a2fb466067f6f59785c527))

## [0.0.17](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.16...v0.0.17) (2026-04-02)


### Features

* add Claude Code skill for worktree task delegation ([52f0d06](https://github.com/DaveDev42/git-worktree-manager/commit/52f0d06d24fb258780c1c7e1549cfacda50cdf94))
* add Claude Code skill for worktree task delegation ([478619b](https://github.com/DaveDev42/git-worktree-manager/commit/478619bf4d95389f473feaed594a3a8ef407ae5a))
* expand skill with full gw reference and references/ support ([aa1a9c1](https://github.com/DaveDev42/git-worktree-manager/commit/aa1a9c123b89b5f3000af3ddaba0434bab2a4b39))

## [0.0.16](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.15...v0.0.16) (2026-04-02)


### Bug Fixes

* prevent env var race condition and config leakage in launch method tests ([378c4eb](https://github.com/DaveDev42/git-worktree-manager/commit/378c4ebd5435d118b4f7b3471b81fc0a49b86fdf))
* prevent env var race condition in launch method tests ([cf0e62f](https://github.com/DaveDev42/git-worktree-manager/commit/cf0e62fef81c03f50dd8f5986f89b095db63de9c))

## [0.0.15](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.14...v0.0.15) (2026-04-02)


### Features

* add dynamic branch name tab completion for all subcommands ([b2d895c](https://github.com/DaveDev42/git-worktree-manager/commit/b2d895cd3c9cf91e40a98d271964b73bc567dc39))
* add SHA256 checksums to release artifacts ([d70336e](https://github.com/DaveDev42/git-worktree-manager/commit/d70336ef9aada8ec58e0c66f53ed873342306bea))
* auto-detect repository default branch ([2e4321c](https://github.com/DaveDev42/git-worktree-manager/commit/2e4321c62c5d4a1274eece01b901c599fcf16a8b))
* auto-upgrade via self-replace from GitHub Releases ([48b948e](https://github.com/DaveDev42/git-worktree-manager/commit/48b948e7a61ebb2826f7006ea19ee5f47b2beb76))
* centralize messages and add format_age tests ([b675c83](https://github.com/DaveDev42/git-worktree-manager/commit/b675c83e085dabfc2548c669d7856c18714870d6))
* code quality — extract helpers, split doctor, shell completion prompt ([faa6eba](https://github.com/DaveDev42/git-worktree-manager/commit/faa6ebad0c4984a24bda9bf8ed79055c4a4f17df))
* code quality refactoring, shell completion parity, CI improvements ([e00938c](https://github.com/DaveDev42/git-worktree-manager/commit/e00938cadad1d214c71c0580769dc1ac7a206293))
* Complete feature parity — all missing commands and options ([908b81c](https://github.com/DaveDev42/git-worktree-manager/commit/908b81c461db3195d3f9d2db949a5d4251c1d169))
* Complete Python feature parity — all 15 items implemented ([e4d86ba](https://github.com/DaveDev42/git-worktree-manager/commit/e4d86babb8b58017db342f3145fe1d06a5eac57b))
* config list table, config key tab completion, cw bash completion ([8ad734f](https://github.com/DaveDev42/git-worktree-manager/commit/8ad734f14dd736d693532f1c47b42a98fcf3f2c2))
* Phase 1 — project scaffolding with core infrastructure ([8bdeb88](https://github.com/DaveDev42/git-worktree-manager/commit/8bdeb887aba1b69c6627ed4d67a9a1e927142eb6))
* Phase 2 — core commands (new, delete, merge, pr, sync, config) ([c310fe6](https://github.com/DaveDev42/git-worktree-manager/commit/c310fe65fc0c8bc198fa4a06f2740517d9021369))
* Phase 3 — terminal launchers and AI tool integration ([5e9d07b](https://github.com/DaveDev42/git-worktree-manager/commit/5e9d07bb3120732bc00d4d78f457f025237db59f))
* Phase 4 — shell functions, auto-update, and upgrade ([e99b7c7](https://github.com/DaveDev42/git-worktree-manager/commit/e99b7c7aeb2424fdf4b58964e06553c00593a817))
* Phase 5 — backup, diagnostics, CI/CD, and polish ([983bdc3](https://github.com/DaveDev42/git-worktree-manager/commit/983bdc37ed8a73fef02bc61ffac76ea8ef637bd7))
* Python parity improvements — CLI flags, config fallback, colored hooks ([a3c8ee2](https://github.com/DaveDev42/git-worktree-manager/commit/a3c8ee2021bd7102a1db8ba8e4f35c875e611726))
* QA improvements — config get, delete --force, hook aliases, backup filtering ([b8a7cc0](https://github.com/DaveDev42/git-worktree-manager/commit/b8a7cc048b4593f935646d21245cccd1cd09dc67))
* refresh shell cache on gw shell-setup ([eebde54](https://github.com/DaveDev42/git-worktree-manager/commit/eebde549cb89446451aa261fe29d0ad45680c504))
* register gw/cw tab completion in shell functions ([1eab90d](https://github.com/DaveDev42/git-worktree-manager/commit/1eab90d615edb5d5803a4fcc854968f1b7bcd740))
* Rename to git-worktree-manager (gw) ([3c061f1](https://github.com/DaveDev42/git-worktree-manager/commit/3c061f1d426eda942664f770ef81fa1a0e4d0aaa))
* rich TUI output, duration parsing, hook error formatting ([7047b87](https://github.com/DaveDev42/git-worktree-manager/commit/7047b873c1175315156613b61d9ff42da27b9c9b))
* show launch method display names in config show/list ([b0f552f](https://github.com/DaveDev42/git-worktree-manager/commit/b0f552f9ed35a93ae2658fb77ea23f6183b97cdb))
* smart config set for ai_tool and launch.method ([79f2102](https://github.com/DaveDev42/git-worktree-manager/commit/79f21026a5a742fe92c739543848e03b5a2c63e7))
* use self_update crate for upgrade, detect Homebrew installs ([eb6aff4](https://github.com/DaveDev42/git-worktree-manager/commit/eb6aff48025f55d03a78213ac861cfe2924cc824))


### Bug Fixes

* Add cw-cd backward compatibility alias ([#9](https://github.com/DaveDev42/git-worktree-manager/issues/9)) ([f2a1361](https://github.com/DaveDev42/git-worktree-manager/commit/f2a13615848dc8883e1cad56351b4e337ab30b13))
* bump MSRV to 1.85, fix rustfmt formatting ([e59ff8e](https://github.com/DaveDev42/git-worktree-manager/commit/e59ff8ec11f28d88175f4b5867d09377c21a13eb))
* check GITHUB_TOKEN/GH_TOKEN env before spawning gh CLI ([e62c3f7](https://github.com/DaveDev42/git-worktree-manager/commit/e62c3f78d87bce74fe7e55f9d756514fd092b9be))
* disable native-tls in self_update to fix cross-compilation ([cf359f5](https://github.com/DaveDev42/git-worktree-manager/commit/cf359f5cf7220775b398f557f9ce72d27e500b5d))
* disable zip default-features to fix MSRV 1.85 and remove time vulnerability ([78ffb28](https://github.com/DaveDev42/git-worktree-manager/commit/78ffb283c609c566d2d50d71adb101ce1db261de))
* pin time and base64ct for MSRV 1.85 compatibility ([edf490d](https://github.com/DaveDev42/git-worktree-manager/commit/edf490dbbe8e4e72addb949b79e64234de786c21))
* remove invalid [warnings] section from audit.toml ([67433d3](https://github.com/DaveDev42/git-worktree-manager/commit/67433d316de723256e3fc503cf3516a6dd93d91c))
* replace self_update with reqwest[rustls-native-roots] for enterprise MDM TLS support ([cb12a30](https://github.com/DaveDev42/git-worktree-manager/commit/cb12a30aff4315857460e577572e0a8fcc2dbd81))
* restore audit.toml ignores for zip→time and indicatif→number_prefix ([ec6ceed](https://github.com/DaveDev42/git-worktree-manager/commit/ec6ceedf0c71d5e8644af8cc1a10ebd1cbedc39a))
* revert version to 0.0.1 so Release Please creates v0.0.2 ([addd37f](https://github.com/DaveDev42/git-worktree-manager/commit/addd37f57d33d8bab48fe5583411dfdc2df1aa96))
* skip Unix-only tests on Windows ([79ce973](https://github.com/DaveDev42/git-worktree-manager/commit/79ce973f96902d9351ebea05d3edbfb6d6bda24a))
* use gh auth token for GitHub API to avoid rate limits ([ef4e498](https://github.com/DaveDev42/git-worktree-manager/commit/ef4e4987b185ea79b1df14a281f16f2d7fd3da35))
* Windows CI — UNC path comparison + cargo fmt for tests ([2a3495e](https://github.com/DaveDev42/git-worktree-manager/commit/2a3495e350623bc0c6070beaeea903bf5effae7b))


### Performance Improvements

* zero-latency startup update check with background refresh ([cb6550d](https://github.com/DaveDev42/git-worktree-manager/commit/cb6550d296bb9beebb5ffa58674629c0efbb108b))

## [0.0.14](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.13...v0.0.14) (2026-04-02)


### Features

* add dynamic branch name tab completion for all subcommands ([b2d895c](https://github.com/DaveDev42/git-worktree-manager/commit/b2d895cd3c9cf91e40a98d271964b73bc567dc39))

## [0.0.13](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.12...v0.0.13) (2026-03-25)


### Bug Fixes

* disable zip default-features to fix MSRV 1.85 and remove time vulnerability ([78ffb28](https://github.com/DaveDev42/git-worktree-manager/commit/78ffb283c609c566d2d50d71adb101ce1db261de))
* remove invalid [warnings] section from audit.toml ([67433d3](https://github.com/DaveDev42/git-worktree-manager/commit/67433d316de723256e3fc503cf3516a6dd93d91c))
* replace self_update with reqwest[rustls-native-roots] for enterprise MDM TLS support ([cb12a30](https://github.com/DaveDev42/git-worktree-manager/commit/cb12a30aff4315857460e577572e0a8fcc2dbd81))
* restore audit.toml ignores for zip→time and indicatif→number_prefix ([ec6ceed](https://github.com/DaveDev42/git-worktree-manager/commit/ec6ceedf0c71d5e8644af8cc1a10ebd1cbedc39a))

## [0.0.12](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.11...v0.0.12) (2026-03-24)


### Bug Fixes

* check GITHUB_TOKEN/GH_TOKEN env before spawning gh CLI ([e62c3f7](https://github.com/DaveDev42/git-worktree-manager/commit/e62c3f78d87bce74fe7e55f9d756514fd092b9be))
* use gh auth token for GitHub API to avoid rate limits ([ef4e498](https://github.com/DaveDev42/git-worktree-manager/commit/ef4e4987b185ea79b1df14a281f16f2d7fd3da35))


### Performance Improvements

* zero-latency startup update check with background refresh ([cb6550d](https://github.com/DaveDev42/git-worktree-manager/commit/cb6550d296bb9beebb5ffa58674629c0efbb108b))

## [0.0.11](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.10...v0.0.11) (2026-03-24)


### Features

* auto-detect repository default branch ([2e4321c](https://github.com/DaveDev42/git-worktree-manager/commit/2e4321c62c5d4a1274eece01b901c599fcf16a8b))
* config list table, config key tab completion, cw bash completion ([8ad734f](https://github.com/DaveDev42/git-worktree-manager/commit/8ad734f14dd736d693532f1c47b42a98fcf3f2c2))
* show launch method display names in config show/list ([b0f552f](https://github.com/DaveDev42/git-worktree-manager/commit/b0f552f9ed35a93ae2658fb77ea23f6183b97cdb))
* smart config set for ai_tool and launch.method ([79f2102](https://github.com/DaveDev42/git-worktree-manager/commit/79f21026a5a742fe92c739543848e03b5a2c63e7))

## [0.0.10](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.9...v0.0.10) (2026-03-23)


### Features

* QA improvements — config get, delete --force, hook aliases, backup filtering ([b8a7cc0](https://github.com/DaveDev42/git-worktree-manager/commit/b8a7cc048b4593f935646d21245cccd1cd09dc67))
* rich TUI output, duration parsing, hook error formatting ([7047b87](https://github.com/DaveDev42/git-worktree-manager/commit/7047b873c1175315156613b61d9ff42da27b9c9b))

## [0.0.9](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.8...v0.0.9) (2026-03-23)


### Features

* add SHA256 checksums to release artifacts ([d70336e](https://github.com/DaveDev42/git-worktree-manager/commit/d70336ef9aada8ec58e0c66f53ed873342306bea))
* auto-upgrade via self-replace from GitHub Releases ([48b948e](https://github.com/DaveDev42/git-worktree-manager/commit/48b948e7a61ebb2826f7006ea19ee5f47b2beb76))
* centralize messages and add format_age tests ([b675c83](https://github.com/DaveDev42/git-worktree-manager/commit/b675c83e085dabfc2548c669d7856c18714870d6))
* code quality — extract helpers, split doctor, shell completion prompt ([faa6eba](https://github.com/DaveDev42/git-worktree-manager/commit/faa6ebad0c4984a24bda9bf8ed79055c4a4f17df))
* code quality refactoring, shell completion parity, CI improvements ([e00938c](https://github.com/DaveDev42/git-worktree-manager/commit/e00938cadad1d214c71c0580769dc1ac7a206293))
* Complete feature parity — all missing commands and options ([908b81c](https://github.com/DaveDev42/git-worktree-manager/commit/908b81c461db3195d3f9d2db949a5d4251c1d169))
* Complete Python feature parity — all 15 items implemented ([e4d86ba](https://github.com/DaveDev42/git-worktree-manager/commit/e4d86babb8b58017db342f3145fe1d06a5eac57b))
* Phase 1 — project scaffolding with core infrastructure ([8bdeb88](https://github.com/DaveDev42/git-worktree-manager/commit/8bdeb887aba1b69c6627ed4d67a9a1e927142eb6))
* Phase 2 — core commands (new, delete, merge, pr, sync, config) ([c310fe6](https://github.com/DaveDev42/git-worktree-manager/commit/c310fe65fc0c8bc198fa4a06f2740517d9021369))
* Phase 3 — terminal launchers and AI tool integration ([5e9d07b](https://github.com/DaveDev42/git-worktree-manager/commit/5e9d07bb3120732bc00d4d78f457f025237db59f))
* Phase 4 — shell functions, auto-update, and upgrade ([e99b7c7](https://github.com/DaveDev42/git-worktree-manager/commit/e99b7c7aeb2424fdf4b58964e06553c00593a817))
* Phase 5 — backup, diagnostics, CI/CD, and polish ([983bdc3](https://github.com/DaveDev42/git-worktree-manager/commit/983bdc37ed8a73fef02bc61ffac76ea8ef637bd7))
* Python parity improvements — CLI flags, config fallback, colored hooks ([a3c8ee2](https://github.com/DaveDev42/git-worktree-manager/commit/a3c8ee2021bd7102a1db8ba8e4f35c875e611726))
* refresh shell cache on gw shell-setup ([eebde54](https://github.com/DaveDev42/git-worktree-manager/commit/eebde549cb89446451aa261fe29d0ad45680c504))
* register gw/cw tab completion in shell functions ([1eab90d](https://github.com/DaveDev42/git-worktree-manager/commit/1eab90d615edb5d5803a4fcc854968f1b7bcd740))
* Rename to git-worktree-manager (gw) ([3c061f1](https://github.com/DaveDev42/git-worktree-manager/commit/3c061f1d426eda942664f770ef81fa1a0e4d0aaa))
* use self_update crate for upgrade, detect Homebrew installs ([eb6aff4](https://github.com/DaveDev42/git-worktree-manager/commit/eb6aff48025f55d03a78213ac861cfe2924cc824))


### Bug Fixes

* Add cw-cd backward compatibility alias ([#9](https://github.com/DaveDev42/git-worktree-manager/issues/9)) ([f2a1361](https://github.com/DaveDev42/git-worktree-manager/commit/f2a13615848dc8883e1cad56351b4e337ab30b13))
* bump MSRV to 1.85, fix rustfmt formatting ([e59ff8e](https://github.com/DaveDev42/git-worktree-manager/commit/e59ff8ec11f28d88175f4b5867d09377c21a13eb))
* disable native-tls in self_update to fix cross-compilation ([cf359f5](https://github.com/DaveDev42/git-worktree-manager/commit/cf359f5cf7220775b398f557f9ce72d27e500b5d))
* pin time and base64ct for MSRV 1.85 compatibility ([edf490d](https://github.com/DaveDev42/git-worktree-manager/commit/edf490dbbe8e4e72addb949b79e64234de786c21))
* revert version to 0.0.1 so Release Please creates v0.0.2 ([addd37f](https://github.com/DaveDev42/git-worktree-manager/commit/addd37f57d33d8bab48fe5583411dfdc2df1aa96))
* skip Unix-only tests on Windows ([79ce973](https://github.com/DaveDev42/git-worktree-manager/commit/79ce973f96902d9351ebea05d3edbfb6d6bda24a))
* Windows CI — UNC path comparison + cargo fmt for tests ([2a3495e](https://github.com/DaveDev42/git-worktree-manager/commit/2a3495e350623bc0c6070beaeea903bf5effae7b))

## [0.0.8](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.7...v0.0.8) (2026-03-23)


### Features

* add SHA256 checksums to release artifacts ([d70336e](https://github.com/DaveDev42/git-worktree-manager/commit/d70336ef9aada8ec58e0c66f53ed873342306bea))

## [0.0.7](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.6...v0.0.7) (2026-03-22)


### Features

* refresh shell cache on gw shell-setup ([eebde54](https://github.com/DaveDev42/git-worktree-manager/commit/eebde549cb89446451aa261fe29d0ad45680c504))

## [0.0.6](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.5...v0.0.6) (2026-03-22)


### Features

* register gw/cw tab completion in shell functions ([1eab90d](https://github.com/DaveDev42/git-worktree-manager/commit/1eab90d615edb5d5803a4fcc854968f1b7bcd740))

## [0.0.5](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.4...v0.0.5) (2026-03-22)


### Bug Fixes

* bump MSRV to 1.85, fix rustfmt formatting ([e59ff8e](https://github.com/DaveDev42/git-worktree-manager/commit/e59ff8ec11f28d88175f4b5867d09377c21a13eb))
* pin time and base64ct for MSRV 1.85 compatibility ([edf490d](https://github.com/DaveDev42/git-worktree-manager/commit/edf490dbbe8e4e72addb949b79e64234de786c21))
* skip Unix-only tests on Windows ([79ce973](https://github.com/DaveDev42/git-worktree-manager/commit/79ce973f96902d9351ebea05d3edbfb6d6bda24a))

## [0.0.4](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.3...v0.0.4) (2026-03-22)


### Features

* auto-upgrade via self-replace from GitHub Releases ([48b948e](https://github.com/DaveDev42/git-worktree-manager/commit/48b948e7a61ebb2826f7006ea19ee5f47b2beb76))
* centralize messages and add format_age tests ([b675c83](https://github.com/DaveDev42/git-worktree-manager/commit/b675c83e085dabfc2548c669d7856c18714870d6))
* code quality — extract helpers, split doctor, shell completion prompt ([faa6eba](https://github.com/DaveDev42/git-worktree-manager/commit/faa6ebad0c4984a24bda9bf8ed79055c4a4f17df))
* code quality refactoring, shell completion parity, CI improvements ([e00938c](https://github.com/DaveDev42/git-worktree-manager/commit/e00938cadad1d214c71c0580769dc1ac7a206293))
* Complete feature parity — all missing commands and options ([908b81c](https://github.com/DaveDev42/git-worktree-manager/commit/908b81c461db3195d3f9d2db949a5d4251c1d169))
* Complete Python feature parity — all 15 items implemented ([e4d86ba](https://github.com/DaveDev42/git-worktree-manager/commit/e4d86babb8b58017db342f3145fe1d06a5eac57b))
* Phase 1 — project scaffolding with core infrastructure ([8bdeb88](https://github.com/DaveDev42/git-worktree-manager/commit/8bdeb887aba1b69c6627ed4d67a9a1e927142eb6))
* Phase 2 — core commands (new, delete, merge, pr, sync, config) ([c310fe6](https://github.com/DaveDev42/git-worktree-manager/commit/c310fe65fc0c8bc198fa4a06f2740517d9021369))
* Phase 3 — terminal launchers and AI tool integration ([5e9d07b](https://github.com/DaveDev42/git-worktree-manager/commit/5e9d07bb3120732bc00d4d78f457f025237db59f))
* Phase 4 — shell functions, auto-update, and upgrade ([e99b7c7](https://github.com/DaveDev42/git-worktree-manager/commit/e99b7c7aeb2424fdf4b58964e06553c00593a817))
* Phase 5 — backup, diagnostics, CI/CD, and polish ([983bdc3](https://github.com/DaveDev42/git-worktree-manager/commit/983bdc37ed8a73fef02bc61ffac76ea8ef637bd7))
* Python parity improvements — CLI flags, config fallback, colored hooks ([a3c8ee2](https://github.com/DaveDev42/git-worktree-manager/commit/a3c8ee2021bd7102a1db8ba8e4f35c875e611726))
* Rename to git-worktree-manager (gw) ([3c061f1](https://github.com/DaveDev42/git-worktree-manager/commit/3c061f1d426eda942664f770ef81fa1a0e4d0aaa))
* use self_update crate for upgrade, detect Homebrew installs ([eb6aff4](https://github.com/DaveDev42/git-worktree-manager/commit/eb6aff48025f55d03a78213ac861cfe2924cc824))


### Bug Fixes

* Add cw-cd backward compatibility alias ([#9](https://github.com/DaveDev42/git-worktree-manager/issues/9)) ([f2a1361](https://github.com/DaveDev42/git-worktree-manager/commit/f2a13615848dc8883e1cad56351b4e337ab30b13))
* disable native-tls in self_update to fix cross-compilation ([cf359f5](https://github.com/DaveDev42/git-worktree-manager/commit/cf359f5cf7220775b398f557f9ce72d27e500b5d))
* revert version to 0.0.1 so Release Please creates v0.0.2 ([addd37f](https://github.com/DaveDev42/git-worktree-manager/commit/addd37f57d33d8bab48fe5583411dfdc2df1aa96))
* Windows CI — UNC path comparison + cargo fmt for tests ([2a3495e](https://github.com/DaveDev42/git-worktree-manager/commit/2a3495e350623bc0c6070beaeea903bf5effae7b))

## [0.0.4](https://github.com/DaveDev42/git-worktree-manager/compare/v0.0.3...v0.0.4) (2026-03-22)


### Features

* auto-upgrade via self-replace from GitHub Releases ([48b948e](https://github.com/DaveDev42/git-worktree-manager/commit/48b948e7a61ebb2826f7006ea19ee5f47b2beb76))
* centralize messages and add format_age tests ([b675c83](https://github.com/DaveDev42/git-worktree-manager/commit/b675c83e085dabfc2548c669d7856c18714870d6))
* code quality — extract helpers, split doctor, shell completion prompt ([faa6eba](https://github.com/DaveDev42/git-worktree-manager/commit/faa6ebad0c4984a24bda9bf8ed79055c4a4f17df))
* code quality refactoring, shell completion parity, CI improvements ([e00938c](https://github.com/DaveDev42/git-worktree-manager/commit/e00938cadad1d214c71c0580769dc1ac7a206293))
* Complete feature parity — all missing commands and options ([908b81c](https://github.com/DaveDev42/git-worktree-manager/commit/908b81c461db3195d3f9d2db949a5d4251c1d169))
* Complete Python feature parity — all 15 items implemented ([e4d86ba](https://github.com/DaveDev42/git-worktree-manager/commit/e4d86babb8b58017db342f3145fe1d06a5eac57b))
* Phase 1 — project scaffolding with core infrastructure ([8bdeb88](https://github.com/DaveDev42/git-worktree-manager/commit/8bdeb887aba1b69c6627ed4d67a9a1e927142eb6))
* Phase 2 — core commands (new, delete, merge, pr, sync, config) ([c310fe6](https://github.com/DaveDev42/git-worktree-manager/commit/c310fe65fc0c8bc198fa4a06f2740517d9021369))
* Phase 3 — terminal launchers and AI tool integration ([5e9d07b](https://github.com/DaveDev42/git-worktree-manager/commit/5e9d07bb3120732bc00d4d78f457f025237db59f))
* Phase 4 — shell functions, auto-update, and upgrade ([e99b7c7](https://github.com/DaveDev42/git-worktree-manager/commit/e99b7c7aeb2424fdf4b58964e06553c00593a817))
* Phase 5 — backup, diagnostics, CI/CD, and polish ([983bdc3](https://github.com/DaveDev42/git-worktree-manager/commit/983bdc37ed8a73fef02bc61ffac76ea8ef637bd7))
* Python parity improvements — CLI flags, config fallback, colored hooks ([a3c8ee2](https://github.com/DaveDev42/git-worktree-manager/commit/a3c8ee2021bd7102a1db8ba8e4f35c875e611726))
* Rename to git-worktree-manager (gw) ([3c061f1](https://github.com/DaveDev42/git-worktree-manager/commit/3c061f1d426eda942664f770ef81fa1a0e4d0aaa))
* use self_update crate for upgrade, detect Homebrew installs ([eb6aff4](https://github.com/DaveDev42/git-worktree-manager/commit/eb6aff48025f55d03a78213ac861cfe2924cc824))


### Bug Fixes

* Add cw-cd backward compatibility alias ([#9](https://github.com/DaveDev42/git-worktree-manager/issues/9)) ([f2a1361](https://github.com/DaveDev42/git-worktree-manager/commit/f2a13615848dc8883e1cad56351b4e337ab30b13))
* revert version to 0.0.1 so Release Please creates v0.0.2 ([addd37f](https://github.com/DaveDev42/git-worktree-manager/commit/addd37f57d33d8bab48fe5583411dfdc2df1aa96))
* Windows CI — UNC path comparison + cargo fmt for tests ([2a3495e](https://github.com/DaveDev42/git-worktree-manager/commit/2a3495e350623bc0c6070beaeea903bf5effae7b))

## [0.0.3](https://github.com/DaveDev42/git-worktree-manager/compare/git-worktree-manager-v0.0.2...git-worktree-manager-v0.0.3) (2026-03-22)


### Features

* auto-upgrade via self-replace from GitHub Releases ([48b948e](https://github.com/DaveDev42/git-worktree-manager/commit/48b948e7a61ebb2826f7006ea19ee5f47b2beb76))

## [0.0.2](https://github.com/DaveDev42/git-worktree-manager/compare/git-worktree-manager-v0.0.1...git-worktree-manager-v0.0.2) (2026-03-22)


### Features

* centralize messages and add format_age tests ([b675c83](https://github.com/DaveDev42/git-worktree-manager/commit/b675c83e085dabfc2548c669d7856c18714870d6))
* code quality — extract helpers, split doctor, shell completion prompt ([faa6eba](https://github.com/DaveDev42/git-worktree-manager/commit/faa6ebad0c4984a24bda9bf8ed79055c4a4f17df))
* code quality refactoring, shell completion parity, CI improvements ([e00938c](https://github.com/DaveDev42/git-worktree-manager/commit/e00938cadad1d214c71c0580769dc1ac7a206293))
* Complete feature parity — all missing commands and options ([908b81c](https://github.com/DaveDev42/git-worktree-manager/commit/908b81c461db3195d3f9d2db949a5d4251c1d169))
* Complete Python feature parity — all 15 items implemented ([e4d86ba](https://github.com/DaveDev42/git-worktree-manager/commit/e4d86babb8b58017db342f3145fe1d06a5eac57b))
* Phase 1 — project scaffolding with core infrastructure ([8bdeb88](https://github.com/DaveDev42/git-worktree-manager/commit/8bdeb887aba1b69c6627ed4d67a9a1e927142eb6))
* Phase 2 — core commands (new, delete, merge, pr, sync, config) ([c310fe6](https://github.com/DaveDev42/git-worktree-manager/commit/c310fe65fc0c8bc198fa4a06f2740517d9021369))
* Phase 3 — terminal launchers and AI tool integration ([5e9d07b](https://github.com/DaveDev42/git-worktree-manager/commit/5e9d07bb3120732bc00d4d78f457f025237db59f))
* Phase 4 — shell functions, auto-update, and upgrade ([e99b7c7](https://github.com/DaveDev42/git-worktree-manager/commit/e99b7c7aeb2424fdf4b58964e06553c00593a817))
* Phase 5 — backup, diagnostics, CI/CD, and polish ([983bdc3](https://github.com/DaveDev42/git-worktree-manager/commit/983bdc37ed8a73fef02bc61ffac76ea8ef637bd7))
* Python parity improvements — CLI flags, config fallback, colored hooks ([a3c8ee2](https://github.com/DaveDev42/git-worktree-manager/commit/a3c8ee2021bd7102a1db8ba8e4f35c875e611726))
* Rename to git-worktree-manager (gw) ([3c061f1](https://github.com/DaveDev42/git-worktree-manager/commit/3c061f1d426eda942664f770ef81fa1a0e4d0aaa))


### Bug Fixes

* Add cw-cd backward compatibility alias ([#9](https://github.com/DaveDev42/git-worktree-manager/issues/9)) ([f2a1361](https://github.com/DaveDev42/git-worktree-manager/commit/f2a13615848dc8883e1cad56351b4e337ab30b13))
* revert version to 0.0.1 so Release Please creates v0.0.2 ([addd37f](https://github.com/DaveDev42/git-worktree-manager/commit/addd37f57d33d8bab48fe5583411dfdc2df1aa96))
* Windows CI — UNC path comparison + cargo fmt for tests ([2a3495e](https://github.com/DaveDev42/git-worktree-manager/commit/2a3495e350623bc0c6070beaeea903bf5effae7b))
