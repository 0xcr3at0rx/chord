# Contributing to Chord

We love contributions! Chord is built for extreme efficiency and simplicity. To maintain these standards, please follow these guidelines.

## Philosophy: No Bloat, Performance First

- **Zero Bloat**: Every new feature or dependency must be strictly necessary. If a feature adds significant complexity without a massive benefit for the majority of users, it will likely be rejected.
- **Performance First**: CPU and RAM usage are our primary metrics. Every PR must include a brief analysis of how it affects these resources.
- **SIMD & Lock-Free**: Prefer SIMD vectorization (using the `wide` crate) and lock-free data structures (like `crossbeam-queue`) for the audio path.
- **Minimal Allocations**: Avoid heap allocations in hot loops. Use `SmolStr` for metadata and pre-allocate buffers where possible.

## Pull Request Guidelines

1.  **Exhaustive Pre-PR Check**: Before submitting, run:
    ```bash
    cargo xtest
    ```
    This runs all unit and integration tests with all features enabled to ensure every edge case is covered.
2.  **Linting & Style**: Run `cargo clippy` and `cargo fmt`. No warnings are allowed.
3.  **Use the PR Template**: Ensure all items in the checklist are addressed.
4.  **Exhaustive Testing**: Add comprehensive tests for new functionality. Your tests MUST cover:
    - **Happy Path**: Expected behavior with valid inputs.
    - **Edge Cases**: Handling of extreme values, empty inputs, or boundary conditions.
    - **Error States**: Graceful recovery from network failures, corrupt metadata, or missing files.
    - **Performance Checks**: Ensure new code doesn't introduce regressions under heavy load.
5.  **Documentation**: Update relevant `.md` files if your change impacts usage or performance.

## Bug Reports & Feature Requests

Please use the provided [GitHub Issue Templates](https://github.com/0xcr3at0rx/chord/issues/new/choose) to ensure all necessary technical details are included.

Thank you for helping keep Chord fast and beautiful!
