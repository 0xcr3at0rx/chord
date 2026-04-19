# Chord

**Chord** is a high-fidelity TUI music player for local audio files. It provides a clean, responsive interface for browsing and playing your music library.

<img align="right" width="300" src="images/screenshot1.png" alt="Chord TUI Screenshot 1">
<img align="right" width="300" src="images/screenshot2.png" alt="Chord TUI Screenshot 2">

## What it does

- **Local Playback**: A TUI music player for FLAC, MP3, WAV, and more.
- **Indexing**: Scans local folders and maintains a database for track access.
- **Album Art**: High-fidelity image preview in the TUI (requires a terminal with image support like Kitty, iTerm2, or WezTerm).
- **Visualizer**: Real-time high-density visualizer with **10 different modes** (Wave, Bars, Matrix, Particles, etc.).
- **Radio Mode**: Stream online radio stations (Ctrl+R). Cycle by country or search all stations.
- **Dynamic Radio Art**: Procedural animated art for live streams.
- **Custom Themes**: Full hex-code support for personalization via the configuration menu (Ctrl+C).

<br clear="right"/>

## How it works

Just run the `chord` command. The app will automatically scan your `music_dir` for files, update its local cache (`library_cache.toml`), and open the TUI player for you to browse and play your music.

## Controls

| Key | Action |
| :--- | :--- |
| `j` / `k` | Navigate lists |
| `Enter` | Play selection |
| `Space` / `p` | Pause / Resume |
| `Ctrl + c` | Configuration Menu |
| `Ctrl + r` | Radio Mode |
| `Tab` | Selection Context (Playlists / Countries) |
| `r` | Rescan / Refresh |
| `Esc` | Return to Normal Mode |

## Configuration

Settings are in `~/.config/chord/config.toml`.

```toml
[library]
music_dir = "~/Music"
scan_at_startup = true

[audio]
visualizer = "Matrix"
volume = 1.0
```

[theme]
bg = "#121212"
# ... hex colors
```

## License

GNU GPL v3. See [LICENSE](LICENSE).