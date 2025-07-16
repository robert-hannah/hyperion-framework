# hyperion-framework

A lightweight Rust framework for building modular, component-based systems with built-in TCP messaging and CLI control.

## Features

- 🔌 **Component-Based Architecture**: Build modular systems with loosely coupled components
- 🌐 **TCP Communication**: Built-in networking support for distributed systems
- 💻 **CLI Integration**: Command-line interface for system control and monitoring
- 🔄 **State Management**: Robust component state handling and lifecycle management
- 📦 **Containerisation**: Simplified component containment and management
- 🚀 **Async Support**: Built on tokio for high-performance async operations

## Installation

Add this to your `Cargo.toml`:

`hyperion-network = 0.3.0`

## Project Structure

- `network/`: TCP communication and networking components
- `messages/`: Message definitions and component directives
- `utilities/`: Common utilities and helper functions
- `data_management/`: Data handling and persistence
- `containerisation/`: Component lifecycle and state management

## Dependencies

- tokio (1.44.2) - Async runtime
- serde (1.0.219) - Serialisation framework
- serde_json (1.0.140) - JSON support
- serde-xml-rs (0.8.1) - XML support
- log (0.4.27) - Logging infrastructure
- colored (3.0.0) - Terminal coloring
- async-trait (0.1.88) - Async trait support

## Documentation

Detailed documentation is available in the `docs/` directory.

## Contributing

Contributions are welcome! Please feel free to submit a PR with a comprehensive description of work done.

## License

Apache 2.0

## Test Pipeline
![hyperion-framework CI](https://github.com/yourusername/hyperion-framework/actions/workflows/ci.yml/badge.svg)