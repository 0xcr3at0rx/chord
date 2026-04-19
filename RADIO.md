# Radio Mode

**Chord** now supports streaming online radio stations directly from the TUI.

## Entering Radio Mode

Press `Ctrl + r` to enter Radio Mode. 

## Features

- **Public Stations**: Includes a set of high-quality default public radio stations.
- **Custom Stations**: Add your own favorite streams in `~/.config/chord/radio.toml`.
- **Filtering**: Press `/` in Radio Mode to search for specific stations.
- **Views**: 
    - **All Radios**: Shows all available stations.
    - **Country-wise**: Press `Tab` to toggle between showing all radios or filtering by the current country.
    - **Cycle Countries**: Use `Left` / `Right` (or `h` / `l`) to cycle through countries when in Country-wise view.

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
| `Tab` | Open Country Selection |
| `j` / `k` | Navigate station list |
| `Enter` | Start streaming selected station |
| `Esc` | Return to Normal Mode |
| `/` | Search/Filter stations |
