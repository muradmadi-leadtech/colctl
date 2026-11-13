# Changelog

## [2.3.0](https://github.com/muradmadi-leadtech/colctl/compare/v2.2.1...v2.3.0) (2026-11-13)


### Features

* **config:** record conversions ([#87](https://github.com/muradmadi-leadtech/colctl/issues/87)) ([22cc5a8](https://github.com/muradmadi-leadtech/colctl/commit/22cc5a8e03f3bd9b00713246676189da4ca1fa13))
* **convert:** hold a Codex package for review ([#88](https://github.com/muradmadi-leadtech/colctl/issues/88)) ([cbc3ddc](https://github.com/muradmadi-leadtech/colctl/commit/cbc3ddcf2b59d29dfbf2fa370e40ac697db2f611))
* **convert:** install conversions in Codex profiles ([#89](https://github.com/muradmadi-leadtech/colctl/issues/89)) ([3ae9339](https://github.com/muradmadi-leadtech/colctl/commit/3ae933995b94aec13dfeb7a027dbd1349794d702))
* **convert:** review a converted source's update with its Codex package ([#90](https://github.com/muradmadi-leadtech/colctl/issues/90)) ([c0b2d04](https://github.com/muradmadi-leadtech/colctl/commit/c0b2d043efa05a6665f2a3122c723a39147ce857))
* **repackage:** build a Codex package from a Claude plugin ([#84](https://github.com/muradmadi-leadtech/colctl/issues/84)) ([4c0f606](https://github.com/muradmadi-leadtech/colctl/commit/4c0f60683a20bbd0fbeba1f5cb6919406861e8db))
* **repackage:** carry over the hooks Codex runs ([#85](https://github.com/muradmadi-leadtech/colctl/issues/85)) ([2d2fd46](https://github.com/muradmadi-leadtech/colctl/commit/2d2fd463c3b112a9a2dee4d7ef35cdded0766b60))


### Bug Fixes

* **repackage:** carry only what Codex reads when there are no hooks ([#92](https://github.com/muradmadi-leadtech/colctl/issues/92)) ([9c89447](https://github.com/muradmadi-leadtech/colctl/commit/9c8944754ecd7bf9dc9df4cd592d7b41fe2c8f91))

## [2.2.1](https://github.com/muradmadi-leadtech/colctl/compare/v2.2.0...v2.2.1) (2026-11-13)


### Bug Fixes

* **output:** profile lines by kind, alike paths, a welcome help ([#79](https://github.com/muradmadi-leadtech/colctl/issues/79)) ([05ca0cd](https://github.com/muradmadi-leadtech/colctl/commit/05ca0cda946fd24ca9bcce51cf7ca24e3ff89dd8))

## [2.2.0](https://github.com/muradmadi-leadtech/colctl/compare/v2.1.2...v2.2.0) (2026-11-13)


### Features

* ask only when it helps, and give every command the same look ([#74](https://github.com/muradmadi-leadtech/colctl/issues/74)) ([38114c5](https://github.com/muradmadi-leadtech/colctl/commit/38114c53ef237477569450e438f923f4a0668c5f))
* grouped help with examples, and shell completions ([#75](https://github.com/muradmadi-leadtech/colctl/issues/75)) ([6499f12](https://github.com/muradmadi-leadtech/colctl/commit/6499f125f1b30ed7cf2e9f049f01263bab8dff30))
* one output layer, global output flags, and errors that say what to do ([#68](https://github.com/muradmadi-leadtech/colctl/issues/68)) ([162b1a4](https://github.com/muradmadi-leadtech/colctl/commit/162b1a40d3f16c0b004b1eb3fd4f0058fab055c6))
* running commands show live progress and say what they did ([#73](https://github.com/muradmadi-leadtech/colctl/issues/73)) ([2e393ef](https://github.com/muradmadi-leadtech/colctl/commit/2e393efdb85179e5bc8406d914f2939ad54779cb))
* snapshot every command's output, fit any width, and document the output system ([#76](https://github.com/muradmadi-leadtech/colctl/issues/76)) ([a14f120](https://github.com/muradmadi-leadtech/colctl/commit/a14f1204d465b9ce556891f60b26a9fd0c6c60a5))
* status leads with a verdict, shows only what needs attention, and lists everything with --all ([#72](https://github.com/muradmadi-leadtech/colctl/issues/72)) ([b660edb](https://github.com/muradmadi-leadtech/colctl/commit/b660edb68e4c265003b7910e6173767bbf2c1b9c))


### Bug Fixes

* keep short ids whole next to long ones, and start stacked outcomes on the profile line ([#77](https://github.com/muradmadi-leadtech/colctl/issues/77)) ([716cc66](https://github.com/muradmadi-leadtech/colctl/commit/716cc6619d1401425e238eb96d817dd33ba525ff))

## [2.1.2](https://github.com/muradmadi-leadtech/colctl/compare/v2.1.1...v2.1.2) (2026-11-13)


### Bug Fixes

* give Codex wrapper marketplaces a local folder source ([#69](https://github.com/muradmadi-leadtech/colctl/issues/69)) ([d41493d](https://github.com/muradmadi-leadtech/colctl/commit/d41493da617fd8b420ffa1a66e3f91620dbc0759))

## [2.1.1](https://github.com/muradmadi-leadtech/colctl/compare/v2.1.0...v2.1.1) (2026-11-13)


### Bug Fixes

* install source plugins from the pinned clone even when the repo's manifest points elsewhere ([#66](https://github.com/muradmadi-leadtech/colctl/issues/66)) ([1a9a76e](https://github.com/muradmadi-leadtech/colctl/commit/1a9a76ed56362626b4338d41985cd26ddf7972c2))

## [2.1.0](https://github.com/muradmadi-leadtech/colctl/compare/v2.0.0...v2.1.0) (2026-11-13)


### Features

* add a source plugin straight from a search result ([#60](https://github.com/muradmadi-leadtech/colctl/issues/60)) ([4b295e9](https://github.com/muradmadi-leadtech/colctl/commit/4b295e9b78b4162e30ef669c7331a9abd23b2db4))
* list and refresh marketplaces, and search Codex connectors ([#59](https://github.com/muradmadi-leadtech/colctl/issues/59)) ([6a69346](https://github.com/muradmadi-leadtech/colctl/commit/6a6934611b18fed552bc7e1a0dfef2ff652dc852))
* merge, match and rank catalog entries ([#57](https://github.com/muradmadi-leadtech/colctl/issues/57)) ([f48c9a0](https://github.com/muradmadi-leadtech/colctl/commit/f48c9a0aba028da70e7bf746a185d785cc852fc4))
* read every plugin catalog the harnesses keep ([#55](https://github.com/muradmadi-leadtech/colctl/issues/55)) ([d981e43](https://github.com/muradmadi-leadtech/colctl/commit/d981e433cf79fe6fb876888f4e4a1e18721f92c3))
* search, show and browse developers across every catalog ([#58](https://github.com/muradmadi-leadtech/colctl/issues/58)) ([93dd67c](https://github.com/muradmadi-leadtech/colctl/commit/93dd67cfef1d53868727081bc09837156d2e529b))


### Bug Fixes

* use a plugin's own name, its source's version, and exit quietly on a closed pipe ([#62](https://github.com/muradmadi-leadtech/colctl/issues/62)) ([43f4005](https://github.com/muradmadi-leadtech/colctl/commit/43f400522313acb1cb52e964311212c4552378ac))

## [2.0.0](https://github.com/muradmadi-leadtech/colctl/compare/v1.2.1...v2.0.0) (2026-11-12)


### ⚠ BREAKING CHANGES

* status shows pending reviews and marks list plugins unreviewed ([#49](https://github.com/muradmadi-leadtech/colctl/issues/49))
* refresh holds changed origins for review instead of copying them ([#45](https://github.com/muradmadi-leadtech/colctl/issues/45))
* update holds new commits for review instead of installing them ([#44](https://github.com/muradmadi-leadtech/colctl/issues/44))

### Features

* approve held candidates exactly as reviewed ([#47](https://github.com/muradmadi-leadtech/colctl/issues/47)) ([657b84b](https://github.com/muradmadi-leadtech/colctl/commit/657b84bde2708210538cfbdbc6f25fc7f1d235ba))
* flag risky changes in held candidates ([#42](https://github.com/muradmadi-leadtech/colctl/issues/42)) ([5602c1f](https://github.com/muradmadi-leadtech/colctl/commit/5602c1fdb42a9236aec1963597c65802cd1c6171))
* refresh holds changed origins for review instead of copying them ([#45](https://github.com/muradmadi-leadtech/colctl/issues/45)) ([2531791](https://github.com/muradmadi-leadtech/colctl/commit/2531791d718c3148ff32d13d19f8416d65eac8d5))
* reject held candidates and drop them when an item is removed ([#48](https://github.com/muradmadi-leadtech/colctl/issues/48)) ([40f60b9](https://github.com/muradmadi-leadtech/colctl/commit/40f60b94e30e3f5cf72685a4f76b67883cda9d0c))
* review held candidates with their diff and risk flags ([#46](https://github.com/muradmadi-leadtech/colctl/issues/46)) ([272edd9](https://github.com/muradmadi-leadtech/colctl/commit/272edd9cd41cda642d0e5ec996be381a82c9434f))
* status shows pending reviews and marks list plugins unreviewed ([#49](https://github.com/muradmadi-leadtech/colctl/issues/49)) ([2fd67b8](https://github.com/muradmadi-leadtech/colctl/commit/2fd67b82989a9f41f5feed5feb33ed6159423a49))
* store held candidates and keep their commits ([#41](https://github.com/muradmadi-leadtech/colctl/issues/41)) ([f934fca](https://github.com/muradmadi-leadtech/colctl/commit/f934fca726d5d2f2f45aa214b5f4e34dc76c59df))
* update holds new commits for review instead of installing them ([#44](https://github.com/muradmadi-leadtech/colctl/issues/44)) ([b0ba530](https://github.com/muradmadi-leadtech/colctl/commit/b0ba530cbd16456bf2b1afbf12d1a2333a0a5712))

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
