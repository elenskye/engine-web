# Kestrel

Kestrel is a small **task runner** for monorepos. It reads `kestrel.toml`, works out which packages changed, and runs only the tasks that need to run.

## Install

```sh
cargo install kestrel --locked
```

## Usage

1. Add a `kestrel.toml` at the repository root.
2. Run `kestrel run test` to test the packages that changed since `main`.
3. Add `--all` to ignore the change detection.

### Options

| Flag | Meaning |
| --- | --- |
| `--all` | run every package |
| `--jobs N` | run at most N tasks at once |

See the [configuration guide](docs/config.md) for every key.

> Kestrel never runs scripts from packages you did not list.
