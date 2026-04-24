# Chord Project Development Plan & Architectural Roadmap

## Core Vision
Chord is a high-performance, high-fidelity terminal music player. It focuses on bit-perfect audio playback, a beautiful TUI, and efficient library management.

---

## Implementation Phases

### Phase 1: Foundation (COMPLETED)
- [x] **Core Player**: Rodio-based playback with visualizer integration.
- [x] **Robust Logging**: Tracing-based logger.
- [x] **Library Management**: Fast indexing and cache-based browsing.
- [x] **Radio Support**: Online radio streaming with custom station support.

### Phase 2: High-Fidelity Audio (IN PROGRESS)
- [ ] **Bit-Perfect Engine**: Implement a raw PCM pipeline that bypasses OS mixers where possible.
- [x] **Visualizer Performance**: SIMD-optimized FFT (via `rustfft`) and parallelized TUI rendering with bitwise XOR math optimizations.
- [ ] **Lyrics Support**: Improved LRC parsing and synchronized display.

### Phase 3: Intelligence & Optimization
- [x] **High Efficiency**: Massive CPU and RAM optimizations using SIMD, SmolStr, and lock-free data structures.
- [ ] **Smart Caching**: Predictive pre-fetching of next tracks in the queue.
- [ ] **Plugin System**: VST/AU support for local DSP processing.

---

## Verification & Quality Assurance
- [x] **Performance Benchmarks**: Verified low CPU/Memory footprint even with large libraries (10k+ tracks).
- [ ] **Audio Quality Tests**: Verify bit-perfect output using loopback recording.

## Technical Debt & Maintenance
- [ ] **Refactoring**: Clean up UI components for better modularity.
- [ ] **Documentation**: Comprehensive guide for keybindings and configuration.
