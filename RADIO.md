# Online Mode

**Chord** now supports streaming online radio stations directly from the TUI with an improved, text-based interface.

## Entering Online Mode

Press `Ctrl + r` to enter or toggle Online Mode.

## Features

- **Public Stations**: Includes over 40 high-quality default public radio stations from curated global directories.
- **Custom Stations**: Add your own favorite streams in `~/.config/chord/radio.toml`.
- **Filtering**: Press `/` in Online Mode to search for specific stations by name or country.
- **Country Selection**: Press `Tab` to open the Country list and filter stations by region.
- **Dynamic Art**: Procedural animated art that reflects the station's energy and amplitude in real-time.

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
| `Ctrl` + `r` | Enter Online Mode / Toggle |
| `Tab` | Open Country Selection list |
| `j` / `k` | Navigate station list |
| `Enter` | Start streaming selected station |
| `Esc` | Return to Offline Mode |
| `/` | Search / Filter stations |
| `p` / `Space` | Pause / Resume current stream |
| `v` | Cycle Visualizer Mode |
| `+` / `-` | Control volume |
