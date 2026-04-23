# Chord 🎶

A modern, fast, and beautiful terminal music player and radio streamer built with Rust.

![Chord Screenshot](images/screenshot1.png)
![Chord Playlist](images/screenshot2.png)
![Chord Visualizer](images/screenshot3.png)

## Features
- **Local Library**: Effortlessly play your music with a clean, searchable interface.
- **Radio Streaming**: Stream your favorite stations from around the world.
- **High-Fidelity Visuals**: Advanced CRT Oscilloscope visualizer with phosphor glow and multi-shape geometric waveforms.
- **Intelligent Audio Analysis**: Real-time DSP and FFT analysis for audio-responsive physics and beat detection.
- **Performance Optimized**: Parallelized TUI rendering and bitwise math optimizations for low CPU usage even at high frame rates.
- **Procedural Art**: Unique, colorful, theme-aware art for every radio station and track.
- **Automatic Library Scanner**: Automatically indexes your music directory on startup.
- **Fast & Light**: Optimized for performance and low resource usage.

## Quick Start
1. **Install dependencies**: `alsa-lib` (Linux).
2. **Run**: `cargo run`.
3. **Switch to Radio**: Press `CTRL+R`.
4. **Search**: Press `/`.

## Keybindings
- `j/k` or `Arrows`: Navigate items.
- `l/h`: Skip to Next/Previous track.
- `o/p`: Adjust Volume (Down/Up).
- `Space`: Toggle Play/Pause.
- `Enter`: Play selected item.
- `/`: Toggle Search mode.
- `Tab`: Cycle between library views.
- `q`: Quit.

*For more details, see [KEYBINDINGS.md](./KEYBINDINGS.md) and [REFER.md](./REFER.md).*
