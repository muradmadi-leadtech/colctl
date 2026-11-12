# Changelog

## [1.2.1](https://github.com/muradmadi-leadtech/colctl/compare/v1.2.0...v1.2.1) (2026-11-12)


### Bug Fixes

* adopt plugins already in a list when their source is known ([#35](https://github.com/muradmadi-leadtech/colctl/issues/35)) ([995f16e](https://github.com/muradmadi-leadtech/colctl/commit/995f16eab2c9a070f1764e7cf6e85be242ea9bd1))
* commit the plugins/ ignore rule on the first sync ([#37](https://github.com/muradmadi-leadtech/colctl/issues/37)) ([6d45ea7](https://github.com/muradmadi-leadtech/colctl/commit/6d45ea7f86b0cfb77096146fbcfdbf1e22753004))

## [1.2.0](https://github.com/muradmadi-leadtech/colctl/compare/v1.1.0...v1.2.0) (2026-11-12)


### Features

* add, remove, update and adopt source plugins ([#31](https://github.com/muradmadi-leadtech/colctl/issues/31)) ([93bbae9](https://github.com/muradmadi-leadtech/colctl/commit/93bbae9be6bff1b58614ac4ca0a9f6315a29b076))
* declare source plugins in config ([#24](https://github.com/muradmadi-leadtech/colctl/issues/24)) ([ed70f3a](https://github.com/muradmadi-leadtech/colctl/commit/ed70f3a6fa4f4ecf0586fe0e0e1d4c1495b8bab7))
* detect plugin packaging and write missing manifests ([#27](https://github.com/muradmadi-leadtech/colctl/issues/27)) ([9843eba](https://github.com/muradmadi-leadtech/colctl/commit/9843eba185bfd19b10c663db0648f047be0deefc))
* find where an installed plugin comes from ([#29](https://github.com/muradmadi-leadtech/colctl/issues/29)) ([efe7b55](https://github.com/muradmadi-leadtech/colctl/commit/efe7b5534398470cef6097bdbc49b25d718ed915))
* install source plugins from their clone in every harness ([#30](https://github.com/muradmadi-leadtech/colctl/issues/30)) ([9eb339a](https://github.com/muradmadi-leadtech/colctl/commit/9eb339a90e2aad92c9d65f43cf08cf0eb7417aca))
* keep source plugin clones at their pin ([#25](https://github.com/muradmadi-leadtech/colctl/issues/25)) ([154aaf1](https://github.com/muradmadi-leadtech/colctl/commit/154aaf1180f26c64d9e6b76fdba8bda81a9d60ff))


### Bug Fixes

* leave project-scoped plugins alone and show pending replacements ([#33](https://github.com/muradmadi-leadtech/colctl/issues/33)) ([0236f7d](https://github.com/muradmadi-leadtech/colctl/commit/0236f7d5dae0ac0b80ebf8cf899fd31e598d3eac))

## [1.1.0](https://github.com/muradmadi-leadtech/colctl/compare/v1.0.0...v1.1.0) (2026-11-12)


### Features

* add codex and agents harness kinds and a codex plugin list ([#15](https://github.com/muradmadi-leadtech/colctl/issues/15)) ([19440c2](https://github.com/muradmadi-leadtech/colctl/commit/19440c2ab25ae1f441839840dd90abeb566546a9))
* codex commands, taking over linked skills, and refresh ([#19](https://github.com/muradmadi-leadtech/colctl/issues/19)) ([cd0475e](https://github.com/muradmadi-leadtech/colctl/commit/cd0475e833eca126ce876cdaa926eb3e65eb65a7))
* convert agents to codex custom agents ([#14](https://github.com/muradmadi-leadtech/colctl/issues/14)) ([b702013](https://github.com/muradmadi-leadtech/colctl/commit/b702013387158b7e1b4051b8ff018cb210084ad2))
* read codex profiles and the shared agents folder ([#17](https://github.com/muradmadi-leadtech/colctl/issues/17)) ([cf30b38](https://github.com/muradmadi-leadtech/colctl/commit/cf30b383434acd6eb691f5fdb2bd5f30fa5da309))
* sync codex profiles and the shared agents folder ([#18](https://github.com/muradmadi-leadtech/colctl/issues/18)) ([f526e4e](https://github.com/muradmadi-leadtech/colctl/commit/f526e4e9fa9555b9ea518fba8041997bb7ec7000))

## [1.0.0](https://github.com/muradmadi-leadtech/colctl/compare/v0.1.0...v1.0.0) (2026-11-12)


### Features

* add the config, state, filesystem and git core ([#2](https://github.com/muradmadi-leadtech/colctl/issues/2)) ([c4952ac](https://github.com/muradmadi-leadtech/colctl/commit/c4952ac48a4b540078198f8cabe2c37d86969d0a))
* sync claude profiles with init, status, sync, add, rm and adopt ([#4](https://github.com/muradmadi-leadtech/colctl/issues/4)) ([0b930bf](https://github.com/muradmadi-leadtech/colctl/commit/0b930bf92292b698bfeecfa3c75114a2756cd966))


### Bug Fixes

* never read a skill's clone url as a git option ([#7](https://github.com/muradmadi-leadtech/colctl/issues/7)) ([21d966c](https://github.com/muradmadi-leadtech/colctl/commit/21d966cd69b6efde9249002ae57d6a29bfcb9112))
