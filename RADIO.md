# Radio Mode

**Chord** now supports streaming online radio stations directly from the TUI with an improved, text-based interface.

## Entering Radio Mode

Press `Ctrl + r` to enter or toggle Radio Mode.

## Features

- **Public Stations**: Includes over 40 high-quality default public radio stations from curated global directories.
- **Custom Stations**: Add your own favorite streams in `~/.config/chord/radio.toml`.
- **Filtering**: Press `/` in Radio Mode to search for specific stations by name or country.
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

## Radio Controls

| Key | Action |
| :--- | :--- |
| `Ctrl` + `r` | Enter Radio Mode / Toggle |
| `Tab` | Open Country Selection list |
| `j` / `k` | Navigate station list |
| `Enter` | Start streaming selected station |
| `Esc` | Return to Normal Mode |
| `/` | Search / Filter stations |
| `p` / `Space` | Pause / Resume current stream |
| `+` / `-` | Control volume |
