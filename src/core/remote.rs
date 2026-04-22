use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::RwLock;
use prost::Message;
use bytes::{BytesMut, Buf};
use std::time::Duration;
use std::net::SocketAddr;
use std::collections::HashMap;

pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/remote.rs"));
}

pub use pb::{RemoteCommand, RemoteStatus, DeviceInfo, RemoteEvent, BrowseResponse, SyncResponse};
pub use pb::remote_command::Command;
pub use pb::remote_event::Event;

pub struct RemoteManager {
    pub status: Arc<RwLock<RemoteStatus>>,
    pub discovered_devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    pub device_id: String,
    pub device_name: String,
    pub disable_broadcast: bool,
}

const TCP_PORT: u16 = 44444;
const UDP_PORT: u16 = 44445;

impl RemoteManager {
    pub fn new(device_id: String, device_name: String, disable_broadcast: bool) -> Self {
        Self {
            status: Arc::new(RwLock::new(RemoteStatus {
                device_id: device_id.clone(),
                device_name: device_name.clone(),
                ..Default::default()
            })),
            discovered_devices: Arc::new(RwLock::new(HashMap::new())),
            device_id,
            device_name,
            disable_broadcast,
        }
    }

    #[tracing::instrument(skip(self, cmd_tx, index))]
    pub async fn start_services(
        &self,
        cmd_tx: mpsc::UnboundedSender<Command>,
        index: Arc<crate::storage::index::LibraryIndex>,
    ) -> anyhow::Result<()> {
        tracing::info!("Starting remote management services");
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let discovered_devices = self.discovered_devices.clone();

        // 1. UDP Discovery Broadcaster
        if !self.disable_broadcast {
            let broadcast_id = device_id.clone();
            let broadcast_name = device_name.clone();
            tokio::spawn(async move {
                tracing::debug!("Starting UDP discovery broadcaster");
                let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
                socket.set_broadcast(true).unwrap();
                let broadcast_addr: SocketAddr = format!("255.255.255.255:{}", UDP_PORT).parse().unwrap();
                
                let info = DeviceInfo {
                    id: broadcast_id,
                    name: broadcast_name,
                    address: String::new(), // Will be filled by receiver
                    port: TCP_PORT as u32,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    role: pb::DeviceRole::Combo as i32,
                };

                let mut buf = Vec::new();
                info.encode(&mut buf).unwrap();

                loop {
                    tracing::trace!("Sending discovery broadcast");
                    let _ = socket.send_to(&buf, broadcast_addr).await;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
        } else {
            tracing::info!("UDP discovery broadcast is disabled via config");
        }

        // 2. UDP Discovery Listener
        let listener_id = device_id.clone();
        tokio::spawn(async move {
            tracing::debug!("Starting UDP discovery listener");
            let socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_PORT)).await.unwrap();
            let mut buf = [0u8; 2048];
            loop {
                if let Ok((len, addr)) = socket.recv_from(&mut buf).await {
                    if let Ok(mut info) = DeviceInfo::decode(&buf[..len]) {
                        if info.id != listener_id {
                            tracing::info!(id = %info.id, name = %info.name, addr = %addr, "Discovered new device");
                            info.address = addr.ip().to_string();
                            discovered_devices.write().await.insert(info.id.clone(), info);
                        }
                    }
                }
            }
        });

        // 3. TCP Control Server
        tracing::info!(port = TCP_PORT, "Starting TCP control server");
        let listener = TcpListener::bind(format!("0.0.0.0:{}", TCP_PORT)).await?;
        let status = self.status.clone();

        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, addr)) = listener.accept().await {
                    tracing::info!(client = %addr, "Accepted new control connection");
                    let cmd_tx = cmd_tx.clone();
                    let status = status.clone();
                    let index = index.clone();
                    tokio::spawn(async move {
                        let mut buf = BytesMut::with_capacity(4096);
                        let mut interval = tokio::time::interval(Duration::from_millis(500));
                        
                        loop {
                            tokio::select! {
                                res = stream.read_buf(&mut buf) => {
                                    match res {
                                        Ok(0) => {
                                            tracing::info!(client = %addr, "Control connection closed by client");
                                            break;
                                        }
                                        Err(e) => {
                                            tracing::error!(client = %addr, error = %e, "Control connection error");
                                            break;
                                        }
                                        Ok(_) => {
                                            while buf.len() >= 4 {
                                                let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
                                                if buf.len() >= 4 + len {
                                                    buf.advance(4);
                                                    let msg_data = buf.split_to(len);
                                                    if let Ok(msg) = RemoteCommand::decode(&msg_data[..]) {
                                                        if let Some(cmd) = msg.command {
                                                            match cmd {
                                                                Command::BrowseRequest(req) => {
                                                                    let res = index.handle_browse_request(req).await;
                                                                    let event = RemoteEvent {
                                                                        event: Some(pb::remote_event::Event::BrowseResponse(res)),
                                                                    };
                                                                    let _ = Self::send_event_to_stream(&mut stream, event).await;
                                                                }
                                                                Command::SyncRequest(req) => {
                                                                    let res = SyncResponse {
                                                                        client_time_us: req.client_time_us,
                                                                        server_time_us: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64,
                                                                    };
                                                                    let event = RemoteEvent {
                                                                        event: Some(pb::remote_event::Event::SyncResponse(res)),
                                                                    };
                                                                    let _ = Self::send_event_to_stream(&mut stream, event).await;
                                                                }
                                                                _ => {
                                                                    tracing::debug!(client = %addr, "Forwarding remote command to app");
                                                                    let _ = cmd_tx.send(cmd);
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                _ = interval.tick() => {
                                    let s = status.read().await;
                                    let event = RemoteEvent {
                                        event: Some(pb::remote_event::Event::Status(s.clone())),
                                    };
                                    if Self::send_event_to_stream(&mut stream, event).await.is_err() {
                                        tracing::warn!(client = %addr, "Failed to send status update, closing connection");
                                        break;
                                    }
                                }
                            }
                        }
                    });
                }
            }
        });

        Ok(())
    }

    async fn send_event_to_stream(stream: &mut TcpStream, event: RemoteEvent) -> anyhow::Result<()> {
        let mut out_buf = BytesMut::with_capacity(event.encoded_len() + 4);
        let len = event.encoded_len() as u32;
        out_buf.extend_from_slice(&len.to_be_bytes());
        event.encode(&mut out_buf)?;
        stream.write_all(&out_buf).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn connect_to_device(&self, info: &DeviceInfo) -> anyhow::Result<TcpStream> {
        let addr = format!("{}:{}", info.address, info.port);
        tracing::info!(target = %addr, name = %info.name, "Connecting to remote device");
        let stream = TcpStream::connect(addr).await?;
        Ok(stream)
    }

    #[tracing::instrument(skip(stream, cmd))]
    pub async fn send_command(stream: &mut TcpStream, cmd: Command) -> anyhow::Result<()> {
        tracing::debug!("Sending command to remote device");
        let msg = RemoteCommand { command: Some(cmd) };
        let mut buf = BytesMut::with_capacity(msg.encoded_len() + 4);
        let len = msg.encoded_len() as u32;
        buf.extend_from_slice(&len.to_be_bytes());
        msg.encode(&mut buf)?;
        stream.write_all(&buf).await?;
        Ok(())
    }
}
