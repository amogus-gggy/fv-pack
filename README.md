# fv-pack — basic system utilities

Rust + GTK4 apps for a Linux distro:

| App | Binary | Description |
|-----|--------|-------------|
| Calendar | `fv-calendar` | Custom month grid with in-cell highlighting (red=holiday, blue=notes, purple=both) + per-day notes in `~/.local/share/fv-calendar/events.json` |
| Notepad | `fv-notepad` | `TextView` editor with New / Open / Save / Save As via `FileDialog` |
| Calculator | `fv-calculator` | Button grid + keyboard entry, hand-written evaluator (no extra deps), history |
| Password manager | `fv-passman` | AES-256-GCM vault (`~/.local/share/fv-passman/vault.json`), Argon2id key, search/copy/show/generate |

> `apt-wrapper` (app-store) intentionally **not** started — awaiting project stack from you.

## Layout

```
fv-pack/
  Cargo.toml            # workspace
  fv-calendar/
  fv-notepad/
  fv-calculator/
  fv-passman/
  assets/*.desktop
```

## System deps (Debian/Ubuntu/Mint)

```sh
sudo apt update
sudo apt install -y libgtk-4-dev libadwaita-1-dev build-essential pkg-config
```

Mint 22.x (noble base) ships GTK 4.14 — matches `gtk4 = "0.10"`.

## Build & run

```sh
cargo build --release
./target/release/fv-calendar
./target/release/fv-notepad
./target/release/fv-calculator
./target/release/fv-passman

# or per-app:
cargo run -p fv-calendar
cargo run -p fv-calculator -- --help # (apps take no args; GTK handles --help)
```

Install:

```sh
sudo install -m755 target/release/fv-* /usr/local/bin/
sudo install -m644 assets/*.desktop /usr/share/applications/
```

## Notes

- Language: Russian by default in all apps, switch via ⚙ → Русский/English.
  Stored per-app in `~/.config/fv-calendar/settings.json`,
  `~/.config/fv-notepad/settings.json`, etc. (`{"lang":"ru"|"en"}`).
- Calendar notes format: `{ "YYYY-MM-DD": ["note", ...] }`.
- Calculator supports `+ - * / % ^ ( )` and unary minus. Example: `(2+3)*4 - 10/2`.
  Unit tests: `cargo test -p fv-calculator` (requires GTK dev libs to compile UI crate).
- Passman: master password ≥ 4 chars (use a strong one — it cannot be recovered).
  Vault re-encrypted with fresh salt+nonce on every save.
- Code: edition 2021, no `unsafe`, GTK via `gtk4` crate (`gtk::gio` reused, no extra `gio` dep).

## Next: apt-wrapper

Tell me the stack (e.g. Rust + GTK4 + apt backend? PackageKit? `apt` CLI parsing?
flatpak support? screenshots/icons source?) and I'll scaffold it to match.
