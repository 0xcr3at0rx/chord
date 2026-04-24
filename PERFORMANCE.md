# Performance & Profiling Guide for Chord

## Optimization Philosophy
Chord is designed for extreme efficiency, targeting low CPU and RAM usage even with music libraries containing 10,000+ tracks. 

### Key Technologies
- **SIMD (`wide` crate)**: Used in `src/core/dsp.rs` to process audio samples in 8-lane batches.
- **Lock-Free Buffers (`crossbeam-queue`)**: Prevents audio thread contention and ALSA underruns.
- **Small String Optimization (`smol_str`)**: Minimizes heap allocations for metadata.
- **Lazy Indexing**: Only basic file info is scanned on startup; full ID3 tags are extracted on-demand.
- **Efficient Allocator (`mimalloc`)**: Reduces memory fragmentation and improves multi-threaded performance.

## Native Build
For maximum performance on your specific machine, always build with native CPU instructions enabled:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## Memory Monitoring
Use `htop` or `valgrind` to monitor RAM usage. With the current "Lazy Metadata" implementation, the base footprint should stay well below 40MB for small libraries and scale linearly (but slowly) for larger ones.
