# Changelog

## [0.2.0](https://github.com/denehoffman/pdg-rs/compare/pdg-rs-v0.1.3...pdg-rs-v0.2.0) (2026-11-25)


### ⚠ BREAKING CHANGES

* **tui:** Tab completes inline filters in particle search. Shift+Tab switches between particle and PDG text search. The filter editor writes its settings into the query.
* **tui:** Ctrl+F replaces F2 for advanced filters. The question mark and slash are reserved for help and search focus in the browser.
* **tui:** Detail shortcuts follow the displayed tab order for the current entry.
* resolve resonance shorthand and unique close particle names
* **cli:** add focused particle detail views
* show matching PDG decay channels in particle searches
* add text prefix search, spelling recovery, and entry context
* **cli:** add shell completions and simplify photon isospin filtering
* make local and offline database access predictable
* honor database editions and default to PDG 2026
* resolve particle names and shared PDG IDs in show
* enforce particle search filter semantics
* **cli:** replace nested search commands with pdg find

### Features

* Add owned particle search hits and selected details ([51e7c39](https://github.com/denehoffman/pdg-rs/commit/51e7c396fb6c0678594d53df06042077abae8ee6))
* Add text prefix search, spelling recovery, and entry context ([8bf8d16](https://github.com/denehoffman/pdg-rs/commit/8bf8d16850d294218898af67aef5305fe5353eee))
* **cli:** Add an interactive terminal browser ([f716100](https://github.com/denehoffman/pdg-rs/commit/f716100528c2a92b30e1a8cfe91c155091e1005a))
* **cli:** Add focused particle detail views ([fa48c38](https://github.com/denehoffman/pdg-rs/commit/fa48c383884646d42db6a54e66122b6451b8f684))
* **cli:** Add shell completions and simplify photon isospin filtering ([1499dfa](https://github.com/denehoffman/pdg-rs/commit/1499dfaa86c80f1b518f40086051a10a6c8ce8b3))
* **cli:** Replace nested search commands with pdg find ([6e10e7d](https://github.com/denehoffman/pdg-rs/commit/6e10e7d914caf49629f6cd84f8a53da824bafae4))
* Honor database editions and default to PDG 2026 ([90a5ba6](https://github.com/denehoffman/pdg-rs/commit/90a5ba6aa4dfc08a9495879f0246f84f1837e1f8))
* Resolve particle names and shared PDG IDs in show ([b187f9b](https://github.com/denehoffman/pdg-rs/commit/b187f9be1759f7c1bfc94f3ac69d1c33c12b6ae7))
* Resolve resonance shorthand and unique close particle names ([fcc99cb](https://github.com/denehoffman/pdg-rs/commit/fcc99cbd232a5af712441e38c42d7b2213c4bccc))
* Share fuzzy particle search across Rust and CLI ([ca7b1d9](https://github.com/denehoffman/pdg-rs/commit/ca7b1d919de4d627de2ad1daedb978fd3d02f414))
* Show matching PDG decay channels in particle searches ([95fcb2d](https://github.com/denehoffman/pdg-rs/commit/95fcb2d9b8fb5c383fef55779032de3619b62c53))
* Sort particle search results by mass ([25a3ad2](https://github.com/denehoffman/pdg-rs/commit/25a3ad269638880b6911b8e0f2cf28e7e11e1d23))
* **tui:** Add entry-specific detail tabs ([4705378](https://github.com/denehoffman/pdg-rs/commit/470537860ac01d525ff6bd9cb3e1689a0d91df59))
* **tui:** Add help and search shortcuts ([ec0227f](https://github.com/denehoffman/pdg-rs/commit/ec0227fd29560d76f791b0e33f29fe27e38cde4d))
* **tui:** Add selectable physics filters and decay navigation ([fe395d6](https://github.com/denehoffman/pdg-rs/commit/fe395d6a16f0cf7be193779cd0e13ed23724d20f))
* **tui:** Autocomplete decay states ([9b0a9bf](https://github.com/denehoffman/pdg-rs/commit/9b0a9bfdc0c91487741d7d94409f62e6ecb6d890))
* **tui:** Copy the active search as a find command ([ae26756](https://github.com/denehoffman/pdg-rs/commit/ae267565ef28342f1dcdfca97d36ed771c0cb0da))
* **tui:** Follow linked PDG entries ([4d0d4b6](https://github.com/denehoffman/pdg-rs/commit/4d0d4b6746eb4b361ea474bee1662c741c37df72))
* **tui:** Support all particle search filters ([0820c27](https://github.com/denehoffman/pdg-rs/commit/0820c27e99a5518ec10678986ffc9524c419d1bf))
* **tui:** Support inline search filters and tab completion ([9079fa7](https://github.com/denehoffman/pdg-rs/commit/9079fa756e1e0a96241a26e74ede23cd69a41662))


### Bug Fixes

* Enforce particle search filter semantics ([c50bd10](https://github.com/denehoffman/pdg-rs/commit/c50bd1089426aaf4b41b9037ac4e318931059c72))
* Make local and offline database access predictable ([40b885c](https://github.com/denehoffman/pdg-rs/commit/40b885c62e7653792e9f3023d67d1a7c10128061))
* Pass strict clippy checks ([ab8cc5c](https://github.com/denehoffman/pdg-rs/commit/ab8cc5cc46ddacd59860006e40b372b061a70e69))
* Remove homepage in Cargo.toml ([5996f66](https://github.com/denehoffman/pdg-rs/commit/5996f66f793f2b155121dff22d66fa83f1a7d87b))
* Resolve aliases consistently in decay filters ([8bd7349](https://github.com/denehoffman/pdg-rs/commit/8bd7349853e275f9826fd7685d49621f2ae177ca))


### Performance Improvements

* Build the text index on demand and limit ranked queries ([4a2d921](https://github.com/denehoffman/pdg-rs/commit/4a2d921d400d0d942496c6ef29c296881ef02b6f))

## [0.1.3](https://github.com/denehoffman/pdg-rs/compare/pdg-rs-v0.1.2...pdg-rs-v0.1.3) (2026-06-18)


### Bug Fixes

* Allow negative mcids in searches and improve search results with aliases and better sorting ([a853f96](https://github.com/denehoffman/pdg-rs/commit/a853f964cae387d97d969d1c1d4682875f22cc24))
* Clippy lints ([a37426f](https://github.com/denehoffman/pdg-rs/commit/a37426fa03320ed48630721bbd2663c48e16a734))

## [0.1.2](https://github.com/denehoffman/pdg-rs/compare/pdg-rs-v0.1.1...pdg-rs-v0.1.2) (2026-05-15)


### Features

* Use a local cache for the database instead of embedding it ([a21793a](https://github.com/denehoffman/pdg-rs/commit/a21793ae9801de0f38e039317f54441cf29f296d))

## [0.1.1](https://github.com/denehoffman/pdg-rs/compare/pdg-rs-v0.1.0...pdg-rs-v0.1.1) (2026-05-15)


### Features

* Add advanced particle search ([32960db](https://github.com/denehoffman/pdg-rs/commit/32960db0ab0bd1c50a5035351d0f61d41f129293))
* Add convenience methods for mass and lifetime ([c98c417](https://github.com/denehoffman/pdg-rs/commit/c98c4171a1b43140e625e37fa27ade868857dcaa))
* Add full text search ([0acaecf](https://github.com/denehoffman/pdg-rs/commit/0acaecfc19ec7ed72ff1536924ee94a31b1771c2))
* Add initial pdg CLI ([81e1a54](https://github.com/denehoffman/pdg-rs/commit/81e1a5474a910ff0c7c5638a91df51498d43fe86))
* Add item hierarchy navigation ([e45ba00](https://github.com/denehoffman/pdg-rs/commit/e45ba007c2ebd2703e44c234756754a2c16d44cb))
* Add particle classification ([716f216](https://github.com/denehoffman/pdg-rs/commit/716f216483822d190dd11dca83f72c21da5aa49b))
* Add particle decay convenience APIs ([a91210b](https://github.com/denehoffman/pdg-rs/commit/a91210bf1fe08eab5b638d92c7f03daca08d23cd))
* Add quantum number conversions ([6754ad3](https://github.com/denehoffman/pdg-rs/commit/6754ad3376168ab595f208250685c6b4cce8d987))
* Attach measurement footnotes ([8adcda6](https://github.com/denehoffman/pdg-rs/commit/8adcda6128a01e166a5a6cf68722ad4677d83147))
* Expand advanced particle search filters ([4eab451](https://github.com/denehoffman/pdg-rs/commit/4eab4517b55024bcf7570894b77ebe62e2fa2c73))
* Expose measurement references ([280e6d9](https://github.com/denehoffman/pdg-rs/commit/280e6d9ee320a8ab498e5804e9f77031704fe41d))
* Expose particle metadata notes ([a753ce0](https://github.com/denehoffman/pdg-rs/commit/a753ce0365ea6a3c5d2bb4fd8b2c66229054566e))
* Expose related decay data ([bd2f9be](https://github.com/denehoffman/pdg-rs/commit/bd2f9be8bbc5d1f989671a01ec6a89ff0e72b18f))
* First commit ([a433d65](https://github.com/denehoffman/pdg-rs/commit/a433d65500d3ca54e5ef91ec156c4d9a552ffe77))
* Pretty formatting on all fields, add more info in measurement displays, add footnotes to text search ([a741fad](https://github.com/denehoffman/pdg-rs/commit/a741fad09ef22b2c2144cc061579b21dca5bce55))
* Refine decay search semantics ([f7fcac3](https://github.com/denehoffman/pdg-rs/commit/f7fcac325b2f98e099bebcb0fffe2de677c3569e))
* Refine particle and show CLI output ([02ccd70](https://github.com/denehoffman/pdg-rs/commit/02ccd70d4b798cee893ca44fac7b2ce8c9fe3086))
* Refine pdg CLI lookup and search ([a03b401](https://github.com/denehoffman/pdg-rs/commit/a03b401319b305a32e787d1c9bf9d35d5173700d))
* Resolve particle properties from section data ([319b494](https://github.com/denehoffman/pdg-rs/commit/319b494f2004b07de9b0467a589de197a503edba))


### Bug Fixes

* Apply derived properties to particle filters ([9328f8e](https://github.com/denehoffman/pdg-rs/commit/9328f8e740366988bbf6e3cf51e80dd537033f1e))
* Expand neutral meson decay families ([1e60011](https://github.com/denehoffman/pdg-rs/commit/1e60011a52b2abb0b82c792b5c8e11a1cc31232d))


### Performance Improvements

* Optimize build for runtime performance ([757b8cd](https://github.com/denehoffman/pdg-rs/commit/757b8cd65900b4b6cabafce5a71a81c59e78c262))
