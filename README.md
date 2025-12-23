# compress-targets

Compress and decompress Tak training data (aka targets).
For mostly-sequential data the compression reduces the size to 2.9%
and for randomly sampled states it's 3.9%.

Use `cargo run --release --bin decompress -- ./compressed-selfplay.bin 6` to decompress the selfplay targets.

You can edit `src/bin/decompress.rs` to adjust the output format.
By default it prints the targets one per line to standard output
so that you can pipe it into whatever you want.

Both `compressed-selfplay.bin` and `compressed-reanalyze.bin` contain over 6900000 state-value-policy triples each.
