# Packaging Moxnotify

Moxnotify ships with two binaries:

* `daemon` – the notification daemon
* `ctl` – the control utility

For compatibility with [moxctl](https://github.com/unixpariah/moxctl), the `ctl` binary should be symlinked to `moxnotifyctl`.

## Building and Renaming

```bash
cargo build --release --bin daemon
mv target/release/daemon target/release/moxnotifyd
```

```bash
cargo build --release --bin ctl
mv target/release/ctl target/release/moxnotify
```

## Installation

Install both binaries to a standard location, such as `/usr/local/bin`:

```bash
install -Dm755 target/release/moxnotifyd /usr/local/bin/moxnotifyd
install -Dm755 target/release/moxnotify /usr/local/bin/moxnotify
ln -sf /usr/local/bin/moxnotify /usr/local/bin/moxnotifyctl
```

## Systemd Integration

A systemd service file is provided in `contrib/systemd/moxnotify.service.in`.

Enable with:

```bash
systemctl --user enable --now moxnotify.service
```
