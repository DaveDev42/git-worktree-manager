# Changelog

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
