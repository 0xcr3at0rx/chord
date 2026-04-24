# Chord 🎶

A modern, fast, and beautiful terminal music player and radio streamer built with Rust.

<p align="center">
  <img src="images/screenshot1.png" alt="Chord Main" width="32%">
  <img src="images/screenshot2.png" alt="Chord Playlist" width="32%">
  <img src="images/screenshot3.png" alt="Chord Visualizer" width="32%">
</p>

## Features
- **Local Library**: Effortlessly play your music with a clean, searchable interface.
- **Radio Streaming**: Stream your favorite stations from around the world.
- **High-Fidelity Visuals**: Advanced CRT Oscilloscope visualizer with phosphor glow and multi-shape geometric waveforms.
- **Intelligent Audio Analysis**: Real-time DSP and FFT analysis for audio-responsive physics and beat detection.
- **Extreme Efficiency**: 
  - **SIMD-accelerated**: Uses `wide` crate for batch audio processing.
  - **Lock-Free**: Truly lock-free audio path using `crossbeam-queue` to eliminate ALSA underruns.
  - **Lazy Indexing**: 10x faster startup with on-demand metadata extraction.
  - **Memory Optimized**: Uses `mimalloc` and `SmolStr` for a tiny footprint.
- **Procedural Art**: Unique, colorful, theme-aware art for every radio station and track.
- **Automatic Library Scanner**: Automatically indexes your music directory on startup.

## Quick Start
1. **Install dependencies**: `alsa-lib` (Linux).
2. **Run**: `cargo run`.
3. **Switch to Radio**: Press `CTRL+R`.
4. **Search**: Press `/`.

## Performance & Profiling
Chord is built for extreme efficiency. If you're developing and want to analyze performance:
1. **Target Native**: Build with `RUSTFLAGS="-C target-cpu=native" cargo build --release`.
2. **Memory**: Use `htop` or `valgrind` to monitor the `mimalloc` allocator performance.
3. **Details**: See [PERFORMANCE.md](./PERFORMANCE.md) for the full optimization guide.

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
