# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Changed `MaildirEntry::copy` and `MaildirEntry::r#move` signature

  Both now return a `Result<Option<PathBuf>>`: `Some` when a maildir entry is moved and `None` when it is not moved (for example, when source and destination path are the same).

## [0.2.1] - 2024-08-14

### Added

- Added `Maildirs::remove_all` function

### Changed

- Changed `Maildir::remove` behaviour

  This function now removes `cur`, `new` and `tmp` folders without removing the root folder of the current Maildir.

## [0.2.0] - 2024-08-13

### Changed

- Changed `Maildirs::iter` item from `Maildir` to `MaildirsEntry`

### Fixed

- Improved Maildir++ support

## [0.1.0] - 2024-08-06

### Added

- Imported code from <https://git.sr.ht/~kmaasrud/maildirpp>
- Added basic Nix support
- Added set of Maildir support
