# Contributing

Contributions are welcome! Please follow these guidelines:

## Development Setup

```bash
# Enter the dev shell
nix develop

# Build
cargo build

# Run tests
cargo test

# Format check
cargo fmt --all -- --check

# Lint
cargo clippy --all-targets -- -D warnings

# Run locally
cargo run -- --password YOUR_PASSWORD
```

## Submitting Changes

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-change`)
3. Make your changes
4. Ensure `cargo fmt`, `cargo clippy`, and `cargo test` all pass
5. Commit with a clear message
6. Open a pull request

## Reporting Issues

- Open a GitHub issue with your modem model, firmware version, and a description of the problem
- Include relevant log output or metric samples if possible
- Do **not** include passwords, MAC addresses, IP addresses, or serial numbers

## Adding New Metrics

1. Add the parser in `src/parser.rs`
2. Add the metric rendering in `src/metrics.rs`
3. Add a test for the parser
4. Update the README metrics table
5. Update the Grafana dashboard if applicable
