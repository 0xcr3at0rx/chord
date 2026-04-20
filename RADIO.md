# Online Mode

**Chord** supports streaming online radio stations directly from the TUI.

## Entering Online Mode

Press `Ctrl + r` to toggle between **Library Mode** and **Online Mode**.

## Features

- **Public Stations**: Includes high-quality default public radio stations.
- **Custom Stations**: Add your own favorite streams in `~/.config/chord/radio.toml`.
- **Filtering**: Press `/` in Online Mode to search for specific stations by name or country. Press `/` again or `Enter` to exit search.
- **Visualizer**: Real-time high-fidelity **Wave** visualizer syncs with live streams.

## Custom Configuration (`radio.toml`)

Create a `radio.toml` file in your config directory (`~/.config/chord/`) to manage your personal stations:

```toml
[[stations]]
name = "My Custom Radio"
url = "http://example.com/stream.mp3"
country = "Global"
tags = "Jazz, Relax"

[[stations]]
name = "Techno FM"
url = "https://anotherstream.com/live"
country = "Germany"
tags = "Electronic, Techno"
```

## Online Controls

| Key | Action |
| :--- | :--- |
| `Ctrl` + `r` | Toggle Online Mode / Return to Library |
| `j` / `k` | Navigate station list |
| `Enter` | Start streaming selected station |
| `/` | Toggle Search / Filter stations |
| `p` / `Space` | Pause / Resume current stream |
| `+` / `-` | Control volume |
