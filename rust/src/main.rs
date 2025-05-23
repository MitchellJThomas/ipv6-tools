use anyhow::{Context, Result};
use log::{error, info};
use std::net::SocketAddrV6;
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use pnet::datalink;
use chrono::Local;

const DEFAULT_PORT: u16 = 8080;
const DEFAULT_ADDR: &str = "::1";

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
        let (mut reader, mut writer) = stream.into_split();
        let mut buffer = [0u8; 1024];

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    info!("Client disconnected");
                    break;
                }
                Ok(n) => {
                    let message = String::from_utf8_lossy(&buffer[..n]);
                    info!("Received from client: {}", message);

                    let response = format!("Rust Server [{}] received: {}", server_addr, message);
                    writer.write_all(response.as_bytes()).await?;
                    writer.flush().await?;
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
        let addr = SocketAddrV6::new(
            self.args.address.as_deref().unwrap_or(DEFAULT_ADDR).parse()?,
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
