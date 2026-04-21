# Implementation Reference

## Architecture
Chord is a high-performance terminal music player and radio streamer built in Rust. It utilizes a multi-threaded architecture to separate audio processing, UI rendering, and library management.

### Key Components
- **Audio Engine (`src/player/audio.rs`)**: Built on `rodio`, handles streaming and local playback with a custom `StreamingReader` for robust network resilience.
- **DSP & FFT (`src/core/dsp.rs`)**: Real-time audio analysis using `realfft`. Calculates amplitude, frequency spectrum, and detects beats.
- **Visualizer (`src/core/visualizer.rs`)**: A custom rendering engine that uses a "shader-like" approach to draw smooth geometric waveforms with phosphor glow effects.
- **Library Index (`src/storage/index.rs`)**: SQLite-backed metadata storage with an automatic recursive disk scanner.

## Visualization Physics
The visualizer uses a kinematic system (`VisualizationState`) to track:
- **Velocity/Acceleration**: Linear motion based on audio amplitude.
- **Angular Motion**: Rotation reacting to the frequency spectrum.
- **Camera Zoom**: Reactive depth based on bass energy.
- **Beat Flash**: Instant luminance peaks on detected beats with exponential decay.

## Radio Art Generation
Unique cover art for radio stations is procedurally generated using a deterministic hash of the station name and tinted with the current theme's accent color. This ensures every station has a consistent, unique visual identity.

## Local Library Scanning
On startup, Chord recursively scans the configured music directory for supported audio formats (MP3, FLAC, OGG, WAV, M4A). Metadata is extracted using the `lofty` crate and cached for fast subsequent loads.
