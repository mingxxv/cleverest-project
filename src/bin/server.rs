use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, error, info, warn};
use minifb::{Key, Window, WindowOptions};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

// Using common modules
use cleverest::common::network;
use cleverest::common::protocol::{ClientMessage, FrameData, ServerMessage};
use cleverest::common::util;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// Bind port
    #[arg(short, long, default_value_t = network::DEFAULT_PORT)]
    port: u16,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug)]
enum ControlMessage {
    // Used to signal display task to shut down
    Shutdown,
    // New frame to display
    NewFrame(FrameData),
}

struct ServerState {
    current_frame: Option<FrameData>,
    connected_clients: usize,
    fps: f64,
    last_frame_time: Instant,
    frames_received: usize,
    total_frames: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logger
    let log_level = if cli.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    
    env_logger::Builder::new()
        .filter_level(log_level)
        .init();

    let version = env!("CARGO_PKG_VERSION");
    info!("Cleverest Server v{} starting", version);

    // Create server address
    let server_addr = format!("{}:{}", cli.bind, cli.port)
        .parse::<SocketAddr>()
        .context("Failed to parse bind address")?;

    info!("Starting server on {}", server_addr);
    
    // For now, skip certificate generation and use dummy certificates
    // In a real application, you'd either generate proper self-signed certs
    // or load them from disk
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let key_der = cert.serialize_private_key_der();
    
    let server_cert = rustls::Certificate(cert_der);
    let server_key = rustls::PrivateKey(key_der);
    
    let server_crypto = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![server_cert], server_key)
        .expect("Failed to create server crypto config");
    
    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(server_crypto));
    
    // Configure the server
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
    server_config.transport = Arc::new(transport_config);
    
    // Create the endpoint
    let endpoint = quinn::Endpoint::server(server_config, server_addr)?;
    
    info!("QUIC server started, listening on {}", server_addr);
    
    // Create channels for communication
    let (control_tx, control_rx) = mpsc::channel(100);
    
    // Shared server state
    let state = Arc::new(Mutex::new(ServerState {
        current_frame: None,
        connected_clients: 0,
        fps: 0.0,
        last_frame_time: Instant::now(),
        frames_received: 0,
        total_frames: 0,
    }));
    
    // Start the display task
    let display_state = state.clone();
    let display_handle = tokio::task::spawn_blocking(move || {
        run_display(control_rx, display_state)
    });
    
    // Accept connections
    while let Some(conn) = endpoint.accept().await {
        let connecting = conn.await;
        match connecting {
            Ok(connection) => {
                info!("New connection from: {}", connection.remote_address());
                
                // Update client count
                {
                    let mut state = state.lock().unwrap();
                    state.connected_clients += 1;
                }
                
                // Handle this connection in a new task
                let conn_control_tx = control_tx.clone();
                let conn_state = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(connection, conn_control_tx, conn_state.clone()).await {
                        error!("Connection error: {}", e);
                    }
                    
                    // Update client count on disconnect
                    let mut state = conn_state.lock().unwrap();
                    state.connected_clients -= 1;
                });
            }
            Err(e) => {
                error!("Connection failed: {}", e);
            }
        }
    }
    
    // Wait for display task to finish
    let _ = display_handle.await;
    
    info!("Server shut down successfully");
    Ok(())
}

async fn handle_connection(
    connection: quinn::Connection,
    control_tx: mpsc::Sender<ControlMessage>,
    state: Arc<Mutex<ServerState>>,
) -> Result<()> {
    // Accept a bi-directional stream
    let (mut send, mut recv) = connection.accept_bi().await?;
    
    info!("Established bi-directional stream with {}", connection.remote_address());
    
    // Wait for client hello
    let mut buffer = vec![0; 1024];
    let n = recv.read(&mut buffer).await?
        .ok_or_else(|| anyhow::anyhow!("Connection closed unexpectedly"))?;
    
    let client_msg: ClientMessage = bincode::deserialize(&buffer[..n])?;
    
    let client_id = match client_msg {
        ClientMessage::Hello { client_id, client_version } => {
            info!("Client hello: {} version {}", client_id, client_version);
            client_id
        }
        _ => {
            return Err(anyhow::anyhow!("Expected hello message from client"));
        }
    };
    
    // Send server hello
    let server_id = format!("cleverest-server-{}", uuid::Uuid::new_v4());
    let server_version = env!("CARGO_PKG_VERSION").to_string();
    
    let hello = ServerMessage::Hello {
        server_id,
        server_version,
    };
    
    let encoded = bincode::serialize(&hello)?;
    send.write_all(&encoded).await?;
    
    // Process incoming messages
    loop {
        let mut buffer = vec![0; 10 * 1024 * 1024]; // 10MB buffer for large frames
        
        match recv.read(&mut buffer).await {
            Ok(Some(0)) => {
                info!("Client closed connection");
                break;
            }
            Ok(Some(n)) => {
                match bincode::deserialize::<ClientMessage>(&buffer[..n]) {
                    Ok(msg) => {
                        match msg {
                            ClientMessage::Frame(frame) => {
                                // Process the frame
                                process_frame(frame, &mut send, &control_tx, &state).await?;
                            }
                            ClientMessage::Goodbye => {
                                info!("Client is closing connection");
                                break;
                            }
                            _ => {
                                warn!("Unexpected message from client");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize client message: {}", e);
                        continue;
                    }
                }
            }
            Ok(None) => {
                info!("Client stream finished");
                break;
            }
            Err(e) => {
                error!("Failed to receive data: {}", e);
                break;
            }
        }
    }
    
    // Send goodbye
    let goodbye = ServerMessage::Goodbye;
    let encoded = bincode::serialize(&goodbye)?;
    let _ = send.write_all(&encoded).await;
    
    info!("Connection handler exiting for {}", client_id);
    Ok(())
}

async fn process_frame(
    frame: FrameData,
    send: &mut quinn::SendStream,
    control_tx: &mpsc::Sender<ControlMessage>,
    state: &Arc<Mutex<ServerState>>,
) -> Result<()> {
    // Update state
    {
        let mut state = state.lock().unwrap();
        state.current_frame = Some(frame.clone());
        state.frames_received += 1;
        state.total_frames += 1;
        
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_frame_time);
        
        if elapsed.as_secs_f64() >= 1.0 {
            state.fps = state.frames_received as f64 / elapsed.as_secs_f64();
            state.frames_received = 0;
            state.last_frame_time = now;
            
            debug!("FPS: {:.2}", state.fps);
        }
    }
    
    // Send frame to display
    control_tx.send(ControlMessage::NewFrame(frame.clone())).await?;
    
    // Send acknowledgment
    let ack = ServerMessage::FrameAck {
        timestamp: frame.timestamp,
        received_at: util::current_timestamp(),
    };
    
    let encoded = bincode::serialize(&ack)?;
    send.write_all(&encoded).await?;
    
    Ok(())
}

fn run_display(
    mut control_rx: mpsc::Receiver<ControlMessage>,
    state: Arc<Mutex<ServerState>>,
) -> Result<()> {
    // Create a window
    let mut window = Window::new(
        "Cleverest Display",
        800, // Default width, will be resized when we get a frame
        600, // Default height, will be resized when we get a frame
        WindowOptions {
            resize: true,
            scale: minifb::Scale::X1,
            ..WindowOptions::default()
        },
    )?;
    
    // Start with a blank screen
    let mut buffer = vec![0; 800 * 600];
    
    // Set a reasonable FPS limit for the display
    window.limit_update_rate(Some(Duration::from_micros(16600))); // ~60 FPS
    
    // Main display loop
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Check for new frames
        while let Ok(msg) = control_rx.try_recv() {
            match msg {
                ControlMessage::Shutdown => {
                    info!("Display shutting down");
                    return Ok(());
                }
                ControlMessage::NewFrame(frame) => {
                    // Resize the window if needed
                    let (current_width, current_height) = window.get_size();
                    if current_width != frame.width as usize || current_height != frame.height as usize {
                        window = Window::new(
                            "Cleverest Display",
                            frame.width as usize,
                            frame.height as usize,
                            WindowOptions {
                                resize: true,
                                scale: minifb::Scale::X1,
                                ..WindowOptions::default()
                            },
                        )?;
                        buffer.resize(frame.width as usize * frame.height as usize, 0);
                    }
                    
                    // Convert the frame data to the format expected by minifb
                    match frame.format {
                        cleverest::common::protocol::PixelFormat::BGRA => {
                            // Convert BGRA to ARGB (what minifb expects)
                            for (i, pixel) in buffer.iter_mut().enumerate() {
                                let idx = i * 4;
                                if idx + 3 < frame.data.len() {
                                    let b = frame.data[idx] as u32;
                                    let g = frame.data[idx + 1] as u32;
                                    let r = frame.data[idx + 2] as u32;
                                    let a = frame.data[idx + 3] as u32;
                                    
                                    *pixel = (a << 24) | (r << 16) | (g << 8) | b;
                                }
                            }
                        }
                        _ => {
                            warn!("Unsupported pixel format: {:?}", frame.format);
                        }
                    }
                }
            }
        }
        
        // Update window title with status
        {
            let state = state.lock().unwrap();
            let title = format!(
                "Cleverest Display - FPS: {:.2} - Clients: {}",
                state.fps, state.connected_clients
            );
            window.set_title(&title);
        }
        
        // Update the display
        if let Err(e) = window.update_with_buffer(&buffer, window.get_size().0, window.get_size().1) {
            error!("Failed to update window: {}", e);
            break;
        }
    }
    
    info!("Display window closed");
    Ok(())
}

// End of implementation