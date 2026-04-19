# Chord

**Chord** is a high-fidelity TUI music player for local audio files. It provides a clean, responsive interface for browsing and playing your music library.

<img align="right" width="300" src="images/screenshot1.png" alt="Chord TUI Screenshot 1">
<img align="right" width="300" src="images/screenshot2.png" alt="Chord TUI Screenshot 2">

## What it does

- **Local Playback**: A TUI music player for FLAC, MP3, WAV, and more.
- **Indexing**: Scans local folders and maintains a database for track access.
- **Album Art**: High-fidelity image preview in the TUI (requires a terminal with image support like Kitty, iTerm2, or WezTerm).
- **Metadata**: Shows technical specs (bitrate, sample rate, etc.) and lyrics from files.
- **Visualizer**: Real-time high-density visualizer that reacts to playback.

<br clear="right"/>

## How it works

1.  **Indexing**: The `index` command scans folders, gets metadata, and updates the local cache.
2.  **Playback**: The `play` command opens the TUI player to browse and play your local files.

## Installation

```bash
chmod +x install.sh
./install.sh
```
Or use the `Makefile`:

- `make setup`: Install dependencies.
- `make build`: Build the player.
- `make test`: Run tests.
- `make lint`: Run linting.

## Configuration

Settings are in `~/.config/chord/config.toml`.

```toml
[library]
music_dir = "~/Music"
scan_at_startup = true

[format]
filename_format = "{artist}/{year} - {album}/{track}. {title}"

[audio]
device_name = "ALSA: Default"
volume = 1.0
```

## Commands

- `chord play`: Open the TUI player.
- `chord index`: Update local index.
- `chord dedup`: Scan for and handle duplicate tracks.

## License

GNU GPL v3. See [LICENSE](LICENSE).
