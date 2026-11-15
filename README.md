# New-Paste (raste)

Clipboard manager written in Rust (egui). On macOS it lives as a panel docked to the bottom of the screen.

## Install (macOS, Apple Silicon)

1. Open the [latest release](../../releases/latest) and download `New-Paste-macos-arm64.zip`.
2. Unzip it and move `New-Paste.app` to `/Applications`.
3. The app is not notarized, so remove the quarantine flag once, then launch it:
   ```sh
   xattr -cr /Applications/New-Paste.app
   open /Applications/New-Paste.app
   ```
4. For automatic pasting, allow New-Paste in **System Settings → Privacy & Security → Accessibility**.
   The permission has to be granted again after every update.

## Usage

| Key | Action |
|---|---|
| Shift+Cmd+V | show / hide the panel (works over full-screen apps) |
| ← / → | select a clip |
| Enter, Cmd+1…9 | paste the clip into the previous app |
| Space | preview / edit |
| Cmd+F | search |
| Cmd+[ / Cmd+] | switch pinboards |
| Esc | hide the panel |
| Cmd+Q | quit |

Clipboard history is stored unencrypted in
`~/Library/Application Support/io.artcraft.new-paste/new-paste.db`.

## Build from source

```sh
cargo run -p new-paste                 # run
scripts/bundle-macos.sh --install      # build New-Paste.app and install to /Applications
```

The docked panel, global shortcut and auto-paste are implemented for macOS only.
