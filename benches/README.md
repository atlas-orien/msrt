# Benchmarks

Benchmarks are planned but intentionally not implemented in the v0.1 scaffold.

SRT is a low-level transport library, so performance testing will matter once the real wire codec, engine state machine, reliability policies, and buffer strategies exist.

## Future Benchmark Groups

- `wire_encode`
- `wire_decode`
- `wire_resync`
- `crc16`
- `message_fragment`
- `message_reassembly`
- `engine_tick`
- `retransmit_scan`
- `dedup_window`

## Host Benchmarks

Host benchmarks may use Criterion later. These benchmarks are useful for regression tracking on normal operating systems, but they do not represent MCU timing directly.

## MCU Benchmarks

MCU benchmarks should use target-specific measurement tools, such as cycle counters, probe-based runners, or board-specific timing harnesses.

The benchmark suite should avoid assuming heap allocation unless the benchmark is explicitly measuring an allocation-enabled profile.
