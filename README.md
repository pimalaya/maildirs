# 📫 Maildir++ [![crates.io](https://img.shields.io/crates/v/maildirpp)](https://crates.io/crates/maildirpp) [![docs.rs](https://img.shields.io/docsrs/maildirpp)](https://docs.rs/maildirpp/latest/maildirpp/)

Rust library to manipulate [Maildir++](http://www.courier-mta.org/imap/README.maildirquota.html) emails.

This library is a fork from the great [maildir](https://github.com/staktrace/maildir) library that removes [mailparse](https://github.com/staktrace/mailparse) (which make the lib email parser agnostic) and removes mutable borrows. See [#35](https://github.com/staktrace/maildir/issues/35) and [#37](https://github.com/staktrace/maildir/issues/37).

## Development

The development environment is managed by [Nix](https://nixos.org/download.html). Running `nix-shell` will spawn a shell with everything you need to get started with the lib: `cargo`, `cargo-watch`, `rust-bin`, `rust-analyzer`, `notmuch`…

```sh
# Start a Nix shell
$ nix-shell

# then build the lib
$ cargo build -p maildirpp
```

## Contributing

If you find a **bug** that [does not exist yet](https://todo.sr.ht/~soywod/pimalaya), please send an email at [~soywod/pimalaya@todo.sr.ht](mailto:~soywod/pimalaya@todo.sr.ht).

If you have a **question**, please send an email at [~soywod/pimalaya@lists.sr.ht](mailto:~soywod/pimalaya@lists.sr.ht).

If you want to **propose a feature** or **fix a bug**, please send a patch at [~soywod/pimalaya@lists.sr.ht](mailto:~soywod/pimalaya@lists.sr.ht) using [git send-email](https://git-scm.com/docs/git-send-email) (see [this guide](https://git-send-email.io/) on how to configure it).

If you want to **subscribe** to the mailing list, please send an email at [~soywod/pimalaya+subscribe@lists.sr.ht](mailto:~soywod/pimalaya+subscribe@lists.sr.ht).

If you want to **unsubscribe** to the mailing list, please send an email at [~soywod/pimalaya+unsubscribe@lists.sr.ht](mailto:~soywod/pimalaya+unsubscribe@lists.sr.ht).

If you want to **discuss** about the project, feel free to join the [Matrix](https://matrix.org/) workspace [#pimalaya](https://matrix.to/#/#pimalaya:matrix.org) or contact me directly [@soywod](https://matrix.to/#/@soywod:matrix.org).

## Credits

[![nlnet](https://nlnet.nl/logo/banner-160x60.png)](https://nlnet.nl/project/Himalaya/index.html)

Special thanks to the [nlnet](https://nlnet.nl/project/Himalaya/index.html) foundation that helped Himalaya to receive financial support from the [NGI Assure](https://www.ngi.eu/ngi-projects/ngi-assure/) program of the European Commission in September, 2022.

## Sponsoring

[![GitHub](https://img.shields.io/badge/-GitHub%20Sponsors-fafbfc?logo=GitHub%20Sponsors&style=flat-square)](https://github.com/sponsors/soywod)
[![PayPal](https://img.shields.io/badge/-PayPal-0079c1?logo=PayPal&logoColor=ffffff&style=flat-square)](https://www.paypal.com/paypalme/soywod)
[![Ko-fi](https://img.shields.io/badge/-Ko--fi-ff5e5a?logo=Ko-fi&logoColor=ffffff&style=flat-square)](https://ko-fi.com/soywod)
[![Buy Me a Coffee](https://img.shields.io/badge/-Buy%20Me%20a%20Coffee-ffdd00?logo=Buy%20Me%20A%20Coffee&logoColor=000000&style=flat-square)](https://www.buymeacoffee.com/soywod)
[![Liberapay](https://img.shields.io/badge/-Liberapay-f6c915?logo=Liberapay&logoColor=222222&style=flat-square)](https://liberapay.com/soywod)
