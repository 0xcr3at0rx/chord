# Chord

**Chord** is a high-fidelity TUI music player for local audio files and internet radio. It features a minimalist, distraction-free interface designed for audiophiles who value both aesthetics and performance.

## Preview

<p align="center">
  <img src="images/screenshot1.png" width="800" alt="Chord TUI Main Interface">
  <br>
  <i>The ultra-minimalist "MUSIC" mode with real-time Wave visualizer</i>
</p>

<p align="center">
  <img src="images/screenshot2.png" width="800" alt="Chord TUI Radio Mode">
  <br>
  <i>"RADIO" mode featuring curated global stations and procedural radio art</i>
</p>

## How it works

Just run the `chord` command. The app will automatically scan your `music_dir` for files, update its local cache, and open the TUI player. Chord is built with a focus on simplicity—no top bars, no clutter, just your music and a high-density visualizer.

## Controls

| Key | Action |
| :--- | :--- |
| `j` / `k` | Navigate lists |
| `Enter` | Play selection / Confirm search |
| `Space` / `p` | Pause / Resume |
| `l` / `h` | Next / Previous track |
| `Tab` | Context Select (Library folders) |
| `/` | Toggle Search / Filter |
| `Ctrl + r` | Toggle Online (Radio) Mode |
| `+` / `-` | Volume Control |

## Configuration

Settings are managed in `~/.config/chord/config.toml`.

### High-Fidelity Audio Setup
```toml
[audio]
visualizer = "Wave"
sample_rate = 96000     # Supports 44100 to 192000
buffer_ms = 100         # Lower for latency, higher for stability
resample_quality = 4    # 1 (Fastest) to 4 (Best quality)
bit_depth = 32          # 16 or 32 (Float PCM)
volume = 1.0

[library]
music_dir = "~/music"
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
