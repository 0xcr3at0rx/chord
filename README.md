# Chord

**Chord** is a high-fidelity TUI music player for local audio files and internet radio. It provides a clean, responsive interface for browsing and playing your music library with professional-grade audio options.

<img align="right" width="300" src="images/screenshot1.png" alt="Chord TUI Screenshot 1">
<img align="right" width="300" src="images/screenshot2.png" alt="Chord TUI Screenshot 2">

## Key Features

- **Local Playback**: High-performance playback for FLAC, MP3, WAV, OGG, and more.
- **Audiophile Grade**: Support for high-res output (up to 192kHz), adjustable buffers, and bit-depth control.
- **Visualizer**: Real-time high-density visualizer with **25 different modes** (Matrix, Flame, Pulse, Sakura, etc.).
- **Radio Mode**: Stream over 40+ curated online radio stations (Ctrl+R).
- **Dynamic Radio Art**: Procedural animated art that reacts to live streams.
- **Custom Themes**: Full hex-code support for total personalization.

<br clear="right"/>

## How it works

Just run the `chord` command. The app will automatically scan your `music_dir` for files, update its local cache (`library_cache.toml`), and open the TUI player for you to browse and play your music.

## Controls

| Key | Action |
| :--- | :--- |
| `j` / `k` | Navigate lists |
| `Enter` | Play selection (in Library/Online) |
| `Space` / `p` | Pause / Resume |
| `v` | Cycle Visualizer Mode |
| `Tab` | Context Select (Library folders) |
| `Ctrl + r` | Online Mode |
| `Esc` | Return to Offline Mode |

## Configuration

Settings are managed in `~/.config/chord/config.toml`. Changes to this file take effect on the next startup.

### High-Fidelity Audio Setup
```toml
[audio]
visualizer = "Matrix"
sample_rate = 96000     # Supports 44100 to 192000
buffer_ms = 100         # Lower for latency, higher for stability
resample_quality = 4    # 1 (Fastest) to 4 (Best quality)
bit_depth = 32          # 16 or 32 (Float PCM)
volume = 1.0

[library]
music_dir = "~/Music"
scan_at_startup = true
```

### Theme Customization
```toml
[theme]
bg = "#121212"
fg = "#CCCCCC"
accent = "#1BFD9C"
accent_dim = "#66B2B2"
critical = "#BA0959"
```

## License

GNU GPL v3. See [LICENSE](LICENSE).
