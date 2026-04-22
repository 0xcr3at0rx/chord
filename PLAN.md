# Chord Project Development Plan & Architectural Roadmap

## Core Vision
Chord is a high-performance, high-fidelity multi-device music ecosystem. It focuses on bit-perfect audio streaming, seamless remote control (Spotify Connect style), and collaborative library management over local and wide-area networks.

---

## The "Chord Protocol" (Protobuf Specification)
The Chord Protocol is a custom binary protocol designed for low-latency control and high-bandwidth audio transport. It has been highly optimized for efficiency.

### 1. Discovery & Handshake
- **UDP Broadcast**: Devices announce presence via UDP on port `44445` using the `DeviceInfo` payload (includes `DeviceRole` and protocol `version`).
- **Capability Exchange**: Upon connection, devices exchange a `Capabilities` message:
  - Supported codecs as integers via `AudioFormat` enum (e.g., `PCM`, `FLAC`, `OPUS`) to minimize payload sizes.
  - Max sample rate (up to 768kHz) and bit depth (up to 64-bit float).
  - Hardware info (DAC type, Buffer sizes).

### 2. High-Res Audio Streaming
- **Transport Efficiency**: Audio transmission is split into two phases:
  - **`StreamSetup`**: Negotiates format, sample rate, channels, and bit depth once before playback.
  - **`StreamData`**: Extremely lean packet containing only `stream_id`, `sequence`, `timestamp_us`, and raw `bytes data`. Eliminating strings and redundant format metadata reduces per-packet overhead drastically.
- **Clock Sync**: Packets include `timestamp_us` for PTP-based multi-room synchronization via `SyncRequest`/`SyncResponse`.
- **Zero-Copy**: Protocol designed for direct-to-DAC DMA transfers where hardware supports it.

### 3. Remote Library Management
- **Browsing**: Query remote libraries using `BrowseRequest` (Artist, Album, Playlist) with full pagination (`limit`, `offset`) support.
- **Global Search**: Unified search across all "Chord-Connected" devices.
- **Casting**: Send a local file stream to a remote device seamlessly.

---

## Implementation Phases

### Phase 1: Foundation (COMPLETED)
- [x] **Core Player**: Rodio-based playback with visualizer integration.
- [x] **Multi-Device Discovery**: UDP-based device announcement and discovery.
- [x] **Remote Control**: Basic TCP-based control (Play/Pause/Vol).
- [x] **Robust Logging**: Tracing-based logger with FFI for external integrations.
- [x] **Protocol Efficiency**: Optimized Protobuf schemas with enums and lean stream packets.

### Phase 2: High-Fidelity Streaming (IN PROGRESS)
- [ ] **Bit-Perfect Engine**: Implement a raw PCM pipeline that bypasses OS mixers where possible.
- [ ] **Jitter Buffer**: Dynamic buffering to handle network micro-stutters during high-res streaming.
- [ ] **Codec Integration**: Server-side transcoding (e.g., FLAC to PCM) for legacy devices.

### Phase 3: Advanced Remote Ecosystem
- [ ] **Library Proxy**: Browse and play music from a NAS or another PC running Chord without manual sharing.
- [ ] **Multi-Room Audio**: Synchronized playback across multiple devices with sub-millisecond drift.
- [ ] **Mobile Remote App**: Bridge to Android/iOS via the FFI layer.

### Phase 4: Intelligence & Optimization
- [ ] **Smart Caching**: Predictive pre-fetching of next tracks in the queue.
- [ ] **Network Adaptation**: Auto-switch between High-Res (FLAC) and High-Efficiency (MP3) based on link quality.
- [ ] **Plugin System**: VST/AU support for remote DSP processing.

---

## Technical Debt & Maintenance
- [ ] Refactor `run_app` to use a proper state machine for mode transitions.
- [ ] Implement unit tests for the Protobuf message handlers.
- [ ] Optimize the visualizer for high-refresh-rate displays.
