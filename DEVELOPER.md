# Developer Documentation for Cleverest

## Prerequisites

Before building this application, make sure you have the required system dependencies:

### Fedora Linux
```bash
sudo dnf install libxcb-devel libX11-devel
```

### Ubuntu/Debian
```bash
sudo apt install libxcb1-dev libxcb-shape0-dev libxcb-shm0-dev libxcb-xfixes0-dev libx11-dev
```

## Building and Development

### Building the application

```bash
# Build the entire application
cargo build --features "client server"

# Build only the client
cargo build --features client

# Build only the server
cargo build --features server
```

### Running the application

```bash
# Run the server
cargo run --bin server --features server

# Run the client (connecting to localhost)
cargo run --bin client --features client

# Run the client with custom settings
cargo run --bin client --features client -- --server 192.168.1.100 --port 14389 --fps 30 --quality 80
```

## Technical Details

### Core Components

1. **Client**: Captures the screen using the screenshots crate and sends frames to the server
2. **Server**: Receives frames via QUIC and displays them using minifb

### Protocol

We use QUIC (via the quinn crate) for network communication, which provides:
- Reliable delivery over UDP
- Built-in encryption
- Low latency
- Multiplexing

### Communication Flow

1. Client connects to server and establishes a bi-directional QUIC stream
2. Client and server exchange hello messages with version information
3. Client captures screen frames at the specified FPS
4. Frames are sent to the server
5. Server acknowledges frames and can send configuration requests
6. Either side can send goodbye to terminate the connection

## Future Development

Potential areas for improvement:
- Compression options for different network conditions
- Multi-monitor support
- Proper error recovery
- Better certificate handling
- Region selection for partial screen sharing
- Authentication and authorization