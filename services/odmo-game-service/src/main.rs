use std::sync::Arc;

use anyhow::Context;
use dashmap::DashMap;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tracing::{error, info, warn};

use odmo_application::game::{
    GameApplication, GameFlowError, GameServiceConfig, GameSessionFactory,
};
use odmo_protocol::{GameRequest, PacketReader};

#[derive(Clone)]
struct SessionBroadcast {
    senders: Arc<DashMap<u64, mpsc::Sender<Vec<u8>>>>,
    locations: Arc<DashMap<u64, (i16, u8)>>,
}

impl SessionBroadcast {
    fn new() -> Self {
        Self {
            senders: Arc::new(DashMap::new()),
            locations: Arc::new(DashMap::new()),
        }
    }

    fn register(&self, character_id: u64) -> mpsc::Receiver<Vec<u8>> {
        let (tx, rx) = mpsc::channel(256);
        self.senders.insert(character_id, tx);
        rx
    }

    fn unregister(&self, character_id: u64) {
        self.senders.remove(&character_id);
        self.locations.remove(&character_id);
    }
}

impl odmo_application::BroadcastSink for SessionBroadcast {
    fn send_to(&self, character_id: u64, packet: &[u8]) -> anyhow::Result<()> {
        if let Some(tx) = self.senders.get(&character_id) {
            let _ = tx.try_send(packet.to_vec());
        }
        Ok(())
    }

    fn is_online(&self, character_id: u64) -> bool {
        self.senders.contains_key(&character_id)
    }

    fn send_to_visible(
        &self,
        map_id: i16,
        channel: u8,
        exclude_character_id: u64,
        packet: &[u8],
    ) -> anyhow::Result<()> {
        for entry in self.locations.iter() {
            let cid = *entry.key();
            let (loc_map, loc_chan) = *entry.value();
            if cid != exclude_character_id
                && loc_map == map_id
                && loc_chan == channel
                && let Some(tx) = self.senders.get(&cid)
            {
                let _ = tx.try_send(packet.to_vec());
            }
        }
        Ok(())
    }

    fn send_to_all_except(
        &self,
        exclude_character_id: u64,
        packet: &[u8],
    ) -> anyhow::Result<()> {
        for entry in self.senders.iter() {
            let cid = *entry.key();
            if cid != exclude_character_id {
                let _ = entry.value().try_send(packet.to_vec());
            }
        }
        Ok(())
    }

    fn update_location(&self, character_id: u64, map_id: i16, channel: u8) {
        self.locations.insert(character_id, (map_id, channel));
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .compact()
        .init();

    let bind = std::env::var("ODMO_GAME_BIND").unwrap_or_else(|_| "127.0.0.1:7607".to_string());
    let portal_state_dir = std::env::var("ODMO_PORTAL_STATE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("odmo-portal"));
    let game_server_address =
        std::env::var("ODMO_GAME_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
    let game_server_port: i32 = std::env::var("ODMO_GAME_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7607);

    let broadcast = Arc::new(SessionBroadcast::new());

    let backend = Arc::new(odmo_persistence::initialize_backend().await?);
    let repository = backend.game_repository();

    let app = GameApplication::new(GameServiceConfig { portal_state_dir }, repository)
        .with_broadcast(broadcast.clone())
        .with_game_server(game_server_address, game_server_port);

    let session_factory = GameSessionFactory::new();
    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind game service on {bind}"))?;

    info!("game service listening on {bind}");

    loop {
        let (socket, address) = listener.accept().await?;
        info!("accepted game connection from {address}");
        let app = app.clone();
        let broadcast = broadcast.clone();
        let mut session = session_factory.create();
        tokio::spawn(async move {
            if let Err(error) = serve_client(socket, &app, &mut session, &broadcast).await {
                error!("game session ended with error: {error:#}");
            }
        });
    }
}

async fn serve_client(
    mut socket: TcpStream,
    app: &GameApplication,
    session: &mut odmo_application::game::GameSession,
    broadcast: &SessionBroadcast,
) -> anyhow::Result<()> {
    // Send initial server handshake (opcode -1, 22-byte framed header).
    let mut initial_handshake = vec![0u8; 22];
    initial_handshake[0..4].copy_from_slice(&22u32.to_le_bytes());
    initial_handshake[4..6].copy_from_slice(&(-1i16).to_le_bytes());
    initial_handshake[6..8].copy_from_slice(&session.handshake_seed.to_le_bytes());
    let checksum: u32 = 22u32 ^ 0x2B4D1A3Cu32;
    initial_handshake[18..22].copy_from_slice(&checksum.to_le_bytes());

    info!(
        "sending initial game handshake (22 bytes, seed={}): {:02X?}",
        session.handshake_seed,
        &initial_handshake[..]
    );
    socket.write_all(&initial_handshake).await?;
    socket.flush().await.ok();
    info!("game handshake sent, waiting for client response...");

    let mut session_error: Option<anyhow::Error> = None;
    let mut broadcast_rx: Option<mpsc::Receiver<Vec<u8>>> = None;

    loop {
        // If we have a broadcast receiver, select between socket reads and broadcast packets
        let frame = if let Some(rx) = broadcast_rx.as_mut() {
            tokio::select! {
                result = read_frame(&mut socket) => {
                    match result {
                        Ok(frame) => Some(frame),
                        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => None,
                        Err(error) => {
                            session_error = Some(error.into());
                            None
                        }
                    }
                }
                Some(broadcast_data) = rx.recv() => {
                    // Forward broadcast frame using the 32-bit length/checksum wire header.
                    let modern_data = ensure_modern_frame(&broadcast_data);
                    if let Err(e) = socket.write_all(&modern_data).await {
                        session_error = Some(e.into());
                        None
                    } else {
                        continue;
                    }
                }
            }
        } else {
            read_frame(&mut socket)
                .await
                .ok()
                .map(Some)
                .unwrap_or_else(|| {
                    // If read_frame returned an error, try to distinguish EOF from other errors
                    None
                })
        };

        let frame = match frame {
            Some(f) => f,
            None => break,
        };

        let raw = match PacketReader::from_frame(&frame) {
            Ok(packet) => packet,
            Err(error) => {
                warn!("dropping invalid game packet: {error}");
                continue;
            }
        };
        let raw_packet_type = raw.packet_type;
        let raw_payload_len = raw.payload.len();
        info!("incoming game request opcode={raw_packet_type} payload_bytes={raw_payload_len}");

        let request = match GameRequest::try_from(raw) {
            Ok(request) => request,
            Err(error) => {
                warn!("unsupported game request: {error}");
                continue;
            }
        };

        let is_encyclopedia_load = matches!(request, GameRequest::EncyclopediaLoad);
        if is_encyclopedia_load {
            if raw_packet_type == odmo_protocol::opcode::game::SEAL_REMOVE_LEADER
                && raw_payload_len == 0
            {
                info!("Routing no-payload packet 3233 as Encyclopedia load request.");
            } else {
                info!(
                    "Routing encyclopedia load request packet={} payload_bytes={}",
                    raw_packet_type, raw_payload_len
                );
            }
        }

        match app.handle_request(session, request) {
            Ok(responses) => {
                // Register broadcast channel once character is authenticated
                if let (None, Some(character_id)) = (&broadcast_rx, session.character_id) {
                    let rx = broadcast.register(character_id);
                    broadcast_rx = Some(rx);
                    info!("registered broadcast for character {character_id}");
                }

                for response in responses {
                    let modern = ensure_modern_frame(&response);
                    if let Ok(raw_response) = PacketReader::from_frame(&response) {
                        info!(
                            "Game response sent packet={} payload_bytes={} wire_len={}",
                            raw_response.packet_type,
                            raw_response.payload.len(),
                            modern.len()
                        );
                    }
                    socket.write_all(&modern).await?;
                }
            }
            Err(error) => {
                warn!("game request rejected: {error}");
                if matches!(
                    error,
                    GameFlowError::Unauthenticated | GameFlowError::MissingSessionTicket(_)
                ) {
                    break;
                }
            }
        }
    }

    // Unregister from broadcast on disconnect
    if let Some(character_id) = session.character_id {
        broadcast.unregister(character_id);
    }

    if let Err(error) = app.handle_disconnect(session) {
        warn!("failed to handle game disconnect cleanup: {error}");
    }

    if let Some(error) = session_error {
        Err(error)
    } else {
        Ok(())
    }
}

fn ensure_modern_frame(frame: &[u8]) -> Vec<u8> {
    if frame.len() >= 10 {
        let u32_len = u32::from_le_bytes(frame[0..4].try_into().unwrap()) as usize;
        if u32_len == frame.len() {
            let checksum = u32::from_le_bytes(frame[frame.len() - 4..].try_into().unwrap());
            if checksum == (u32_len as u32) ^ 0x2B4D1A3C_u32 {
                return frame.to_vec();
            }
        }
    }

    if frame.len() >= 6 {
        let opcode = &frame[2..4];
        let payload = &frame[4..frame.len() - 2];
        let modern_len = (payload.len() + 10) as u32;
        let mut modern = Vec::with_capacity(modern_len as usize);
        modern.extend_from_slice(&modern_len.to_le_bytes());
        modern.extend_from_slice(opcode);
        modern.extend_from_slice(payload);
        let checksum = modern_len ^ 0x2B4D1A3C_u32;
        modern.extend_from_slice(&checksum.to_le_bytes());
        return modern;
    }

    frame.to_vec()
}

async fn read_frame(socket: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    socket.read_exact(&mut header).await?;
    let length = if header[2] == 0 && header[3] == 0 && u32::from_le_bytes(header) >= 10 {
        u32::from_le_bytes(header) as usize
    } else {
        u16::from_le_bytes([header[0], header[1]]) as usize
    };
    let mut frame = vec![0_u8; length];
    frame[0..4].copy_from_slice(&header);
    socket.read_exact(&mut frame[4..]).await?;
    Ok(frame)
}
