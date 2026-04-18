# Changelog

## [0.1.7](https://github.com/Roberdan/convergio-kernel/compare/v0.1.6...v0.1.7) (2026-04-18)


### Bug Fixes

* prevent UTF-8 boundary panics in input truncation (DoS) ([ef0b4c2](https://github.com/Roberdan/convergio-kernel/commit/ef0b4c2e1c050ab910a51dc9dd900833a280776b))
* security and quality audit pass 2 ([9a90d8e](https://github.com/Roberdan/convergio-kernel/commit/9a90d8ec445a6211606c1da39d490aac1012f01e))

## [0.1.6](https://github.com/Roberdan/convergio-kernel/compare/v0.1.5...v0.1.6) (2026-04-13)


### Bug Fixes

* pass CARGO_REGISTRY_TOKEN to release workflow ([313b6b9](https://github.com/Roberdan/convergio-kernel/commit/313b6b93649086f38314bfefa9ceb4aff2a19f4b))

## [0.1.5](https://github.com/Roberdan/convergio-kernel/compare/v0.1.4...v0.1.5) (2026-04-13)


### Bug Fixes

* add crates.io publishing metadata (description, repository) ([4eba90c](https://github.com/Roberdan/convergio-kernel/commit/4eba90c4a02dac3a3e2615f56eb144f9d2ce5dec))

## [0.1.4](https://github.com/Roberdan/convergio-kernel/compare/v0.1.3...v0.1.4) (2026-04-13)


### Bug Fixes

* regenerate Cargo.lock after SDK bump ([#10](https://github.com/Roberdan/convergio-kernel/issues/10)) ([2764693](https://github.com/Roberdan/convergio-kernel/commit/276469393de59497dacd2d1f83f54de579bee106))

## [0.1.3](https://github.com/Roberdan/convergio-kernel/compare/v0.1.2...v0.1.3) (2026-04-13)


### Features

* adapt convergio-kernel for standalone repo ([2b6d8dc](https://github.com/Roberdan/convergio-kernel/commit/2b6d8dc354b5e5c5852a37fb5448200f010d6f6e))


### Bug Fixes

* **release:** use vX.Y.Z tag format (remove component) ([474e489](https://github.com/Roberdan/convergio-kernel/commit/474e489f5e958315e632bcf04e0de1553bfda414))
* remove unused dependencies (cargo-udeps) ([959115a](https://github.com/Roberdan/convergio-kernel/commit/959115a5358bd402f883d621e1cb6e7081714bcb))
* security audit — SQL injection, path traversal, timeouts, HTML injection ([f6f6cdb](https://github.com/Roberdan/convergio-kernel/commit/f6f6cdbc3f1c646bcfa3d8ac7a0af08e50b0b1a4))


### Documentation

* add .env.example with required environment variables ([#3](https://github.com/Roberdan/convergio-kernel/issues/3)) ([c0cea9b](https://github.com/Roberdan/convergio-kernel/commit/c0cea9ba49225a6f15146d11fa581509828c4a9b))

## [0.1.2](https://github.com/Roberdan/convergio-kernel/compare/convergio-kernel-v0.1.1...convergio-kernel-v0.1.2) (2026-04-12)


### Documentation

* add .env.example with required environment variables ([#3](https://github.com/Roberdan/convergio-kernel/issues/3)) ([c0cea9b](https://github.com/Roberdan/convergio-kernel/commit/c0cea9ba49225a6f15146d11fa581509828c4a9b))

## [0.1.1](https://github.com/Roberdan/convergio-kernel/compare/convergio-kernel-v0.1.0...convergio-kernel-v0.1.1) (2026-04-12)


### Features

* adapt convergio-kernel for standalone repo ([2b6d8dc](https://github.com/Roberdan/convergio-kernel/commit/2b6d8dc354b5e5c5852a37fb5448200f010d6f6e))


### Bug Fixes

* remove unused dependencies (cargo-udeps) ([959115a](https://github.com/Roberdan/convergio-kernel/commit/959115a5358bd402f883d621e1cb6e7081714bcb))
* security audit — SQL injection, path traversal, timeouts, HTML injection ([f6f6cdb](https://github.com/Roberdan/convergio-kernel/commit/f6f6cdbc3f1c646bcfa3d8ac7a0af08e50b0b1a4))

## 0.1.0 (Initial Release)

### Features

- Initial extraction from convergio monorepo
