# Implementation Reference

## Architecture
Chord is a high-performance terminal music player and radio streamer built in Rust. It utilizes a multi-threaded architecture to separate audio processing, UI rendering, and library management.

### Key Components
- **Audio Engine (`src/player/audio.rs`)**: Built on `rodio`, handles streaming and local playback with a custom `StreamingReader` for robust network resilience.
- **DSP & FFT (`src/core/dsp.rs`)**: Real-time audio analysis using `realfft`. Calculates amplitude, frequency spectrum, and detects beats. Optimized with bitwise XOR math and pre-allocated buffers.
- **Visualizer (`src/core/visualizer.rs`)**: A custom rendering engine that uses a "shader-like" approach to draw smooth geometric waveforms with phosphor glow effects. Rendering is parallelized across CPU cores using `rayon`.
- **Library Index (`src/storage/index.rs`)**: TOML-backed metadata storage with an automatic recursive disk scanner. Fast indexing using `lofty` and stable prefix matching for folder-based playlists.

## Visualization Physics
The visualizer uses a kinematic system (`VisualizationState`) to track:
- **Velocity/Acceleration**: Linear motion based on audio amplitude.
- **Angular Motion**: Rotation reacting to the frequency spectrum.
- **Camera Zoom**: Reactive depth based on bass energy.
- **Beat Flash**: Instant luminance peaks on detected beats with exponential decay.

## Performance Optimizations
- **SIMD Vectorization**: Uses the `wide` crate to process audio samples in 8-lane batches (`f32x8`) for amplitude and windowing, maximizing CPU throughput.
- **Small String Optimization (SSO)**: Replaced `String` with `SmolStr` for track metadata and identifiers, reducing heap allocations and memory fragmentation.
- **Lock-Free Buffers**: Uses SPSC (Single Producer Single Consumer) lock-free ring buffers (`ringbuf`) for passing audio data between threads without Mutex overhead.
- **Bitwise XOR Math**: Uses bit-level operations for floating-point absolute value calculations.
- **Parallel Rendering**: The TUI visualizer renders rows in parallel.
- **Pre-allocated FFT Buffers**: All signal processing buffers are allocated once on startup.
- **Arc & Boxed Slices**: Uses `Arc<[T]>` and `Box<[T]>` for immutable lists to eliminate capacity overhead and improve data sharing.

## Radio Art Generation
Unique cover art for radio stations is procedurally generated using a deterministic hash of the station name and tinted with the current theme's accent color. This ensures every station has a consistent, unique visual identity.

## Local Library Scanning
On startup, Chord recursively scans the configured music directory for supported audio formats (MP3, FLAC, OGG, WAV, M4A). Metadata is extracted using the `lofty` crate and cached in `library.toml` for fast subsequent loads.
