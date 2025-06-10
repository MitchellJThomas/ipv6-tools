use anyhow::{Context, Result};
use log::{error, info};
use std::net::{Ipv6Addr, SocketAddrV6};
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use pnet::datalink;
use chrono::Local;

const DEFAULT_PORT: u16 = 8080;
const DEFAULT_ADDR: &str = "::1";
// Define a maximum line length to prevent potential DoS from overly long messages.
const MAX_LINE_LENGTH: usize = 4096;

#[derive(StructOpt, Debug)]
#[structopt(name = "ipv6_tester", about = "IPv6 testing tool for client/server communication")]
struct Args {
    /// Run as server or client
    #[structopt(possible_values = &["server", "client"])]
    mode: String,

    /// IPv6 address (default: ::1)
    #[structopt(short, long)]
    address: Option<String>,

    /// Port number (default: 8080)
    #[structopt(short, long)]
    port: Option<u16>,
}

struct IPv6Tester {
    args: Args,
}

impl IPv6Tester {
    fn new(args: Args) -> Self {
        Self { args }
    }

    fn print_usage() {
        println!("Usage: ipv6_tester <server|client> [--address <ipv6_address>] [--port <port>]");
        println!("  server|client    - Required. Run as server or client");
        println!("  --address        - Optional. IPv6 address (default: {})", DEFAULT_ADDR);
        println!("  --port           - Optional. Port number (default: {})", DEFAULT_PORT);
        
        println!("\nAvailable IPv6 addresses on this host:");
        if let Ok(interfaces) = get_available_ipv6_addresses() {
            for (iface, addr) in interfaces {
                println!("  {}: {}", iface, addr);
            }
        }

        println!("\nExamples:");
        println!("  ipv6_tester server");
        println!("  ipv6_tester server --address 2001:db8:1234:5678::1");
        println!("  ipv6_tester client --address 2001:db8:1234:5678::1 --port 8888");
    }

    async fn run_server(&self, addr: SocketAddrV6) -> Result<()> {
        let listener = TcpListener::bind(addr).await
            .context("Failed to bind to address")?;
        
        info!("Server listening on [{}]:{}", addr.ip(), addr.port());
        self.log_listener_properties(&listener, "server listener");

        loop {
            match listener.accept().await {
                Ok((stream, client_addr)) => {
                    info!("Client connected from: [{}]", client_addr.ip());
                    self.log_stream_properties(&stream, &format!("client connection from [{}]", client_addr.ip()));
                    
                    let server_addr = addr.ip().to_string();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, &server_addr).await {
                            error!("Error handling client: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    async fn handle_client(stream: TcpStream, server_addr: &str) -> Result<()> {
        // Split the stream into a reader and writer for managing I/O.
        let (reader, mut writer) = tokio::io::split(stream);
        // Use BufReader for efficient, newline-delimited reading.
        // This improves message framing by processing data line by line.
        let mut buf_reader = BufReader::new(reader);
        let mut line_buffer = String::new();

        // Loop to continuously read messages from the client.
        loop {
            line_buffer.clear();
            // Read a line from the client. Newline characters demarcate messages.
            match buf_reader.read_line(&mut line_buffer).await {
                Ok(0) => { // Connection closed (EOF)
                    info!("Client disconnected");
                    break;
                }
                Ok(bytes_read) if bytes_read > 0 => { // Successfully read a line with content.
                    // Check if the received line exceeds the defined maximum length.
                    // This is a security measure to prevent resource exhaustion (DoS) from malicious clients.
                    if line_buffer.len() > MAX_LINE_LENGTH {
                        error!(
                            "Client sent message exceeding max length of {} bytes (read {}). Closing connection.",
                            MAX_LINE_LENGTH, line_buffer.len()
                        );
                        // Optionally send a message to the client
                        let error_response = "Error: Message too long. Closing connection.\n";
                        if writer.write_all(error_response.as_bytes()).await.is_err() {
                            // Log if sending this specific error fails, but don't let it panic
                            error!("Failed to send 'message too long' error to client.");
                        }
                        // Ensure flush is attempted, but don't let its error break normal error flow for "message too long"
                        writer.flush().await.ok();
                        break; // Close connection
                    }

                    let message = line_buffer.trim_end_matches(|c| c == '\r' || c == '\n');

                    if message.is_empty() {
                        // This handles lines that become empty after trimming (e.g., client just sent "\n")
                        // Or if the line_buffer itself was empty but read_line returned >0 (should not happen with current logic)
                        // info!("Received empty line from client after trim."); // Optional: log if needed for debugging
                        continue; // Skip processing for effectively empty lines
                    }

                    info!("Received from client: {}", message);

                    let response = format!("Rust Server [{}] received: {}\n", server_addr, message);
                    writer.write_all(response.as_bytes()).await?;
                    writer.flush().await?;
                }
                Ok(_) => { // bytes_read == 0, but not caught by Ok(0) - defensive, should be Ok(0)
                    info!("Client disconnected (or sent empty payload).");
                    break;
                }
                Err(e) => {
                    error!("Error reading from client: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    async fn run_client(&self, addr: SocketAddrV6) -> Result<()> {
        let stream = TcpStream::connect(addr).await
            .context("Failed to connect to server")?;
        
        println!("Connected to server at [{}]:{}", addr.ip(), addr.port());
        self.log_stream_properties(&stream, &format!("client connection to [{}]:{}", addr.ip(), addr.port()));

        let (mut reader, mut writer) = stream.into_split();
        let mut buffer = [0u8; 1024];

        for _ in 0..20 {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S");
            let message = format!("Hello from Rust IPv6 client at {}", now);
            writer.write_all(message.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
            println!("Sent to server: {}", message);

            // Read response
            match reader.read(&mut buffer).await {
                Ok(n) if n > 0 => {
                    let response = String::from_utf8_lossy(&buffer[..n]);
                    println!("Server response: {}", response.trim_end());
                }
                Ok(_) => {
                    println!("Server closed connection");
                    break;
                }
                Err(e) => {
                    eprintln!("Error reading from server: {}", e);
                    break;
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        Ok(())
    }

    fn log_stream_properties(&self, stream: &TcpStream, context: &str) {
        info!("\nSocket properties for {}:", context);
        info!("  Socket family: IPv6");
        info!("  Socket type: SOCK_STREAM");
        info!("  Socket protocol: TCP");
        if let Ok(addr) = stream.peer_addr() {
            info!("  Remote address: {}", addr);
        }
        if let Ok(addr) = stream.local_addr() {
            info!("  Local address: {}", addr);
        }
    }

    fn log_listener_properties(&self, listener: &TcpListener, context: &str) {
        info!("\nSocket properties for {}:", context);
        if let Ok(addr) = listener.local_addr() {
            info!("  Local address: {}", addr);
        }
        info!("  Socket family: IPv6");
        info!("  Socket type: SOCK_STREAM");
        info!("  Socket protocol: TCP");
    }

    async fn run(&self) -> Result<()> {
        let address_str = self.args.address.as_deref().unwrap_or(DEFAULT_ADDR);
        // Parse the address string into an Ipv6Addr.
        // This approach handles potential parsing errors gracefully by returning an error
        // rather than panicking, which improves robustness.
        let ip_addr: Ipv6Addr = match address_str.parse() {
            Ok(ip) => ip,
            Err(e) => {
                return Err(anyhow::anyhow!("Invalid IPv6 address '{}'. {}", address_str, e));
            }
        };

        let addr = SocketAddrV6::new(
            ip_addr,
            self.args.port.unwrap_or(DEFAULT_PORT),
            0,
            0
        );

        match self.args.mode.as_str() {
            "server" => self.run_server(addr).await,
            "client" => self.run_client(addr).await,
            _ => {
                Self::print_usage();
                Ok(())
            }
        }
    }
}

fn get_available_ipv6_addresses() -> Result<Vec<(String, String)>> {
    let mut addresses = Vec::new();
    for iface in datalink::interfaces() {
        for ip in iface.ips {
            if ip.is_ipv6() {
                addresses.push((iface.name.clone(), ip.ip().to_string()));
            }
        }
    }
    Ok(addresses)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger. Be mindful of log verbosity and content in production environments,
    // especially if handling sensitive data or if logs are accessible to unintended parties.
    // For this tool, default anyhow error messages are generally acceptable for debugging.
    env_logger::init();
    
    let args = match Args::from_args_safe() {
        Ok(args) => args,
        Err(_) => {
            IPv6Tester::print_usage();
            return Ok(());
        }
    };

    let tester = IPv6Tester::new(args);
    tester.run().await
}
