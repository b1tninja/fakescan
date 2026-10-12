# Contributing

fakescan is a lab scanner. A change should keep the same profile and the same page stack producing the same bitmaps, and should arrive with a test when it changes that behavior.

## Setup

Stable Rust with rustfmt and clippy (`rust-toolchain.toml` selects them).

```bash
cargo test -p fakescan
pip install -e "./python[dev]"
pytest python/tests -m "not live and not local"
```

`pytest python/tests -m live` needs a fakescan process. `pytest python/tests -m local` starts one.

## Before a pull request

```bash
cargo fmt --all
cargo clippy -p fakescan --all-targets -- -D warnings
cargo test -p fakescan
pytest python/tests -m "not live and not local"
```

CI runs the same fmt, clippy, and test gates on Ubuntu and Windows, then a Docker smoke, a sane-airscan smoke, and the Python lab (offline, then live against compose). See `.github/workflows/ci.yml`.

Do not commit `.env` or tokens.

## License

MIT OR Apache-2.0. Contributions are dual-licensed the same way ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). Anything derived from Microsoft WIA samples must keep its MS-PL notice.
