use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, error, info, warn};
use screenshots::Screen;
use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time;

// Using common modules
use cleverest::common::network;
use cleverest::common::protocol::{ClientMessage, FrameData, ServerMessage};
use cleverest::common::util;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Server address to connect to
    #[arg(short, long, default_value = "127.0.0.1")]
    server: String,

    /// Server port
    #[arg(short, long, default_value_t = network::DEFAULT_PORT)]
    port: u16,

    /// Target frames per second
    #[arg(short, long, default_value_t = 30)]
    fps: u8,

    /// Quality (1-100)
    #[arg(short, long, default_value_t = 80)]
    quality: u8,

    /// Enable active screen capture (false will initialize but not capture frames)
    #[arg(short, long, default_value_t = true)]
    capture: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone)]
struct ClientSettings {
    fps: u8,
    quality: u8,
    active_capture: bool,  // Whether to actively capture frames
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
    info!("Cleverest Client v{} starting", version);

    // Connect to server
    let server_addr = format!("{}:{}", cli.server, cli.port)
        .to_socket_addrs()?
        .next()
        .context("Failed to resolve server address")?;

    info!("Connecting to server at {}", server_addr);
    
    // Create endpoint
    let endpoint = network::create_client_endpoint().await?;
    
    // Connect to server
    let connection = endpoint
        .connect(server_addr, "localhost")?
        .await
        .context("Failed to connect to server")?;
    
    info!("Connected to server successfully!");
    
    // Open a bi-directional stream
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .context("Failed to open bi-directional stream")?;

    // Send hello message
    let hello = ClientMessage::Hello {
        client_id: format!("cleverest-client-{}", uuid::Uuid::new_v4()),
        client_version: version.to_string(),
    };
    
    let encoded = bincode::serialize(&hello)?;
    send.write_all(&encoded).await?;
    
    // Wait for server hello
    let mut buf = vec![0; 1024];
    let n = recv.read(&mut buf).await?
        .ok_or_else(|| anyhow::anyhow!("Connection closed unexpectedly"))?;
    let server_msg: ServerMessage = bincode::deserialize(&buf[..n])?;
    
    match server_msg {
        ServerMessage::Hello { server_id, server_version } => {
            info!("Connected to server {} running version {}", server_id, server_version);
        }
        _ => {
            return Err(anyhow::anyhow!("Expected hello message from server"));
        }
    }

    // Create channels for communication between tasks
    let (frame_tx, mut frame_rx) = mpsc::channel(5); // Buffer a few frames
    let (control_tx, mut control_rx) = mpsc::channel(10);
    
    // Shared settings that can be modified by the server
    let settings = Arc::new(Mutex::new(ClientSettings {
        fps: cli.fps,
        quality: cli.quality,
        active_capture: cli.capture,
    }));
    
    // Spawn capture task
    let capture_settings = settings.clone();
    let capture_handle = tokio::spawn(async move {
        capture_frames(frame_tx, capture_settings).await
    });
    
    // Spawn network send task
    let send_handle = tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            let msg = ClientMessage::Frame(frame);
            match bincode::serialize(&msg) {
                Ok(encoded) => {
                    if let Err(e) = send.write_all(&encoded).await {
                        error!("Failed to send frame: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize frame: {}", e);
                    continue;
                }
            }
        }
        
        // Send goodbye message
        let goodbye = ClientMessage::Goodbye;
        if let Ok(encoded) = bincode::serialize(&goodbye) {
            let _ = send.write_all(&encoded).await;
        }
        
        info!("Sender task shutting down");
    });
    
    // Spawn network receive task
    let recv_settings = settings.clone();
    let recv_handle = tokio::spawn(async move {
        let mut buf = vec![0; 1024 * 1024]; // 1MB buffer for receiving
        
        loop {
            match recv.read(&mut buf).await {
                Ok(Some(0)) => {
                    info!("Server closed connection");
                    break;
                }
                Ok(Some(n)) => {
                    match bincode::deserialize::<ServerMessage>(&buf[..n]) {
                        Ok(msg) => {
                            handle_server_message(msg, &recv_settings, &control_tx).await;
                        }
                        Err(e) => {
                            error!("Failed to deserialize server message: {}", e);
                            continue;
                        }
                    }
                }
                Ok(None) => {
                    info!("Server stream finished");
                    break;
                }
                Err(e) => {
                    error!("Failed to receive data: {}", e);
                    break;
                }
            }
        }
        
        info!("Receiver task shutting down");
        // Signal other tasks to shut down
        let _ = control_tx.send(ControlMessage::Shutdown).await;
    });
    
    // Wait for control message to shut down
    if let Some(ControlMessage::Shutdown) = control_rx.recv().await {
        info!("Shutting down client");
    }
    
    // Wait for tasks to complete
    let _ = capture_handle.await;
    let _ = send_handle.await;
    let _ = recv_handle.await;
    
    info!("Client shut down successfully");
    Ok(())
}

#[derive(Debug)]
enum ControlMessage {
    Shutdown,
    RequestKeyFrame,
}

async fn handle_server_message(
    msg: ServerMessage,
    settings: &Arc<Mutex<ClientSettings>>,
    control_tx: &mpsc::Sender<ControlMessage>,
) {
    match msg {
        ServerMessage::FrameAck { timestamp, received_at } => {
            let now = util::current_timestamp();
            let latency = now.saturating_sub(timestamp);
            debug!("Frame ack: latency {}ms, server received at {}", latency, received_at);
        }
        ServerMessage::RequestKeyFrame => {
            info!("Server requested key frame");
            let _ = control_tx.send(ControlMessage::RequestKeyFrame).await;
        }
        ServerMessage::ConfigureQuality { quality } => {
            info!("Server requested quality change to {}", quality);
            if let Ok(mut settings) = settings.lock() {
                settings.quality = quality;
            }
        }
        ServerMessage::ConfigureFrameRate { fps } => {
            info!("Server requested frame rate change to {}", fps);
            if let Ok(mut settings) = settings.lock() {
                settings.fps = fps;
            }
        }
        ServerMessage::ToggleCapture { enabled } => {
            info!("Server requested capture state change to {}", enabled);
            if let Ok(mut settings) = settings.lock() {
                settings.active_capture = enabled;
            }
        }
        ServerMessage::Goodbye => {
            info!("Server is closing connection");
            let _ = control_tx.send(ControlMessage::Shutdown).await;
        }
        _ => {
            warn!("Unexpected message from server: {:?}", msg);
        }
    }
}

async fn capture_frames(
    frame_tx: mpsc::Sender<FrameData>,
    settings: Arc<Mutex<ClientSettings>>,
) {
    // Get the main screen
    let screens = match Screen::all() {
        Ok(screens) => screens,
        Err(e) => {
            error!("Failed to get screens: {}", e);
            return;
        }
    };
    
    if screens.is_empty() {
        error!("No screens found");
        return;
    }
    
    let screen = &screens[0]; // Use the first screen
    info!(
        "Initializing primary screen: {}x{} at ({}, {})",
        screen.display_info.width,
        screen.display_info.height,
        screen.display_info.x,
        screen.display_info.y
    );
    
    // Take a single screenshot to get initial dimensions and format
    let initial_capture = match screen.capture() {
        Ok(capture) => capture,
        Err(e) => {
            error!("Failed to capture initial screen: {}", e);
            return;
        }
    };
    
    let width = initial_capture.width() as u32;
    let height = initial_capture.height() as u32;
    info!("Screen initialized successfully: {}x{}", width, height);
    
    // Create an empty buffer to send when capture is disabled
    let empty_buffer = vec![0; (width * height * 4) as usize]; // 4 bytes per pixel (BGRA)
    
    let mut last_frame_time = Instant::now();
    
    loop {
        // Check our current settings
        let (fps, active_capture) = {
            if let Ok(settings) = settings.lock() {
                (settings.fps, settings.active_capture)
            } else {
                (30, false) // Default if we can't access settings
            }
        };
        
        // Calculate the delay for the desired FPS
        let frame_delay = Duration::from_secs_f64(1.0 / fps as f64);
        let elapsed = last_frame_time.elapsed();
        
        // Wait if we're ahead of schedule
        if elapsed < frame_delay {
            time::sleep(frame_delay - elapsed).await;
        }
        
        let frame = if active_capture {
            // Only capture the screen if active capture is enabled
            debug!("Capturing frame");
            match screen.capture() {
                Ok(capture) => {
                    // Convert image to our format
                    let width = capture.width() as u32;
                    let height = capture.height() as u32;
                    let buffer = capture.as_raw().to_vec();
                    
                    FrameData {
                        width,
                        height,
                        timestamp: util::current_timestamp(),
                        data: buffer,
                        format: cleverest::common::protocol::PixelFormat::BGRA,
                        compressed: false, // Not implementing compression yet
                        key_frame: true,   // All frames are key frames for now
                    }
                },
                Err(e) => {
                    error!("Failed to capture screen: {}", e);
                    // Send empty frame with timestamp when capture fails
                    FrameData {
                        width,
                        height,
                        timestamp: util::current_timestamp(),
                        data: empty_buffer.clone(),
                        format: cleverest::common::protocol::PixelFormat::BGRA,
                        compressed: false,
                        key_frame: true,
                    }
                }
            }
        } else {
            // Send empty or blank frame when capture is disabled
            debug!("Capture disabled, sending placeholder frame");
            FrameData {
                width,
                height,
                timestamp: util::current_timestamp(),
                data: empty_buffer.clone(),
                format: cleverest::common::protocol::PixelFormat::BGRA,
                compressed: false,
                key_frame: true,
            }
        };
        
        // Send the frame
        if frame_tx.send(frame).await.is_err() {
            // Channel is closed, exit
            break;
        }
        
        last_frame_time = Instant::now();
    }
    
    info!("Frame capture task exiting");
}