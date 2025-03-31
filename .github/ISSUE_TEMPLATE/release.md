---
name: Release
about: Create a new release [for release managers only]
title: 'Release MAJOR.MINOR.PATCH'
labels: 'release'
assignee
---

**Steps** 

- [ ] Update the Kyoto version contained in the User Agent of the P2P Version message.
- [ ] Update the changelog with the recent commit history.
- [ ] Check the documentation for errors using `cargo +nightly doc --open`
- [ ] Increase the version number in `Cargo.toml` according to semver
- [ ] Attempt a publish dry-run using `cargo publish --dry-run`
- [ ] Create a pull request and ensure all tests pass
- [ ] Merge the pull request and publish from `master`

