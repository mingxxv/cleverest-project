# Cleverest

A simple desktop screencast application written in Rust, with separate client and server components.

## Features

- Client captures desktop screen and streams it to the server
- Server displays the streamed screen in real-time
- Uses QUIC protocol for efficient, reliable streaming
- Configurable quality and frame rate
- Efficient image encoding and transmission

## Requirements

- Rust 1.70 or later
- Build tools (gcc, etc.)
- X11 development libraries (for Linux)

### Linux Dependencies

On Debian/Ubuntu:

```bash
sudo apt install libx11-dev libxcb1-dev libxcb-shm0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

On Fedora:

```bash
sudo dnf install libX11-devel libxcb-devel
```

## Building

To build both client and server:

```bash
cd cleverest
cargo build --release --features "client server"
```

To build only the client:

```bash
cargo build --release --features "client"
```

To build only the server:

```bash
cargo build --release --features "server"
```

## Usage

### Running the Server

```bash
# Run with default settings (listen on 0.0.0.0:14389)
cargo run --release --features server -- server

# Specify bind address and port
cargo run --release --features server -- server --bind 192.168.1.100 --port 8080

# Enable verbose logging
cargo run --release --features server -- server --verbose
```

### Running the Client

```bash
# Connect to server on localhost with default port
cargo run --release --features client -- client

# Connect to a specific server
cargo run --release --features client -- client --server 192.168.1.200 --port 8080

# Configure quality and frame rate
cargo run --release --features client -- client --fps 15 --quality 60

# Enable verbose logging
cargo run --release --features client -- client --verbose
```

## Architecture

### Network Protocol

Cleverest uses the QUIC protocol (via the quinn crate) for network communication:

- QUIC provides reliability, congestion control, and multiplexing over UDP
- Lower latency compared to TCP
- Built-in encryption 

### Components

- **Client**: Captures screen frames and streams them to the server
- **Server**: Receives frames from clients and displays them

### Common Code

- **protocol.rs**: Data structures for client-server communication
- **network.rs**: QUIC configuration and setup
- **error.rs**: Error types and handling
- **util.rs**: Utility functions for timestamps, image conversion, etc.

## Limitations

- Currently supports only the primary monitor
- No audio support
- No input control from server to client
- Self-signed certificates for development

## Future Improvements

- Multi-monitor support
- Audio streaming
- Compression options
- Proper certificate handling
- Region selection for partial screen sharing
- Client authorization
- Multiple client support
- Performance optimizations

## License

This project is licensed under the MIT License - see the LICENSE file for details.