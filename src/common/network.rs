use crate::common::error::{CleverestError, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls::{Certificate, PrivateKey};

pub const ALPN_PROTOCOL: &[u8] = b"cleverest";
pub const DEFAULT_PORT: u16 = 14389;

// Generate a self-signed certificate for testing/development
pub fn generate_self_signed_cert() -> Result<(Vec<Certificate>, PrivateKey)> {
    // In a real application, you'd want to generate proper certificates
    // or use a library like rcgen. For now, this is a placeholder.
    Err(CleverestError::Config("Not implemented: Use proper certificates in production".into()))
}

// Set up server configuration
pub fn configure_server(cert: Vec<Certificate>, key: PrivateKey) -> Result<ServerConfig> {
    let mut server_config = ServerConfig::with_single_cert(cert, key)
        .map_err(|e| CleverestError::Config(format!("Failed to configure server: {}", e)))?;
    
    // Set ALPN protocols
    Arc::get_mut(&mut server_config.transport)
        .ok_or_else(|| CleverestError::Config("Failed to get mutable reference to transport config".into()))?
        .keep_alive_interval(Some(std::time::Duration::from_secs(5)));
    
    Ok(server_config)
}

// Set up client configuration
pub fn configure_client() -> Result<ClientConfig> {
    // Insecure for development - don't verify certs
    let crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();
    
    let mut client_config = ClientConfig::new(Arc::new(crypto));
    
    // Set ALPN protocols
    let mut transport = quinn::TransportConfig::default();
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(5)));
    client_config.transport_config(Arc::new(transport));
    
    Ok(client_config)
}

// Create a server endpoint
pub async fn create_server_endpoint(addr: SocketAddr, server_config: ServerConfig) -> Result<Endpoint> {
    let endpoint = Endpoint::server(server_config, addr)
        .map_err(|e| CleverestError::Network(format!("Failed to create server endpoint: {}", e)))?;
    
    Ok(endpoint)
}

// Create a client endpoint
pub async fn create_client_endpoint() -> Result<Endpoint> {
    let client_config = configure_client()?;
    let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
        .map_err(|e| CleverestError::Network(format!("Failed to create client endpoint: {}", e)))?;
    endpoint.set_default_client_config(client_config);
    
    Ok(endpoint)
}

// Dummy certificate verifier that accepts any certificate
struct SkipServerVerification;

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> std::result::Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}