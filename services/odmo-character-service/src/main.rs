use anyhow::Context;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, warn};

use odmo_application::character::{
    CharacterApplication, CharacterFlowError, CharacterServiceConfig, CharacterSessionFactory,
};
use odmo_protocol::{CharacterRequest, PacketReader};
use odmo_types::GameServerTarget;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .compact()
        .init();

    let bind =
        std::env::var("ODMO_CHARACTER_BIND").unwrap_or_else(|_| "127.0.0.1:7050".to_string());
    let game_host = std::env::var("ODMO_GAME_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let game_port = std::env::var("ODMO_GAME_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(7607);
    let portal_state_dir = std::env::var("ODMO_PORTAL_STATE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("odmo-portal"));

    let backend = std::sync::Arc::new(odmo_persistence::initialize_backend().await?);
    let repository = backend.character_repository();
    let account_repository = backend.character_account_repository();

    let app = CharacterApplication::new(
        CharacterServiceConfig {
            game_server: GameServerTarget {
                address: game_host,
                port: game_port,
            },
            portal_state_dir,
        },
        repository,
        account_repository,
    );
    let session_factory = CharacterSessionFactory::new();
    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind character service on {bind}"))?;

    info!("character service listening on {bind}");

    loop {
        let (socket, address) = listener.accept().await?;
        info!("accepted character connection from {address}");
        let app = app.clone();
        let mut session = session_factory.create();
        tokio::spawn(async move {
            if let Err(error) = serve_client(socket, &app, &mut session).await {
                error!("character session ended with error: {error:#}");
            }
        });
    }
}

async fn serve_client(
    mut socket: TcpStream,
    app: &CharacterApplication,
    session: &mut odmo_application::character::CharacterSession,
) -> anyhow::Result<()> {
    // Send initial server handshake (opcode -1, 22-byte framed header).
    let mut initial_handshake = vec![0u8; 22];
    initial_handshake[0..4].copy_from_slice(&22u32.to_le_bytes());
    initial_handshake[4..6].copy_from_slice(&(-1i16).to_le_bytes());
    initial_handshake[6..8].copy_from_slice(&session.handshake_seed.to_le_bytes());
    let checksum: u32 = 22u32 ^ 0x2B4D1A3Cu32;
    initial_handshake[18..22].copy_from_slice(&checksum.to_le_bytes());

    info!(
        "sending initial character handshake (22 bytes, seed={}): {:02X?}",
        session.handshake_seed,
        &initial_handshake[..]
    );
    socket.write_all(&initial_handshake).await?;
    socket.flush().await.ok();
    info!("character handshake sent, waiting for client...");

    loop {
        let frame = match read_frame(&mut socket).await {
            Ok(frame) => frame,
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        };

        let raw = match PacketReader::from_frame(&frame) {
            Ok(packet) => {
                info!(
                    "character packet opcode={} payload_len={}",
                    packet.packet_type,
                    packet.payload.len()
                );
                packet
            }
            Err(error) => {
                warn!("dropping invalid character packet: {error}");
                continue;
            }
        };

        let request = match CharacterRequest::try_from(raw) {
            Ok(request) => {
                info!("parsed character request: {request:?}");
                request
            }
            Err(error) => {
                warn!("unsupported character request: {error}");
                continue;
            }
        };

        match app.handle_request(session, request) {
            Ok(responses) => {
                info!("sending {} character response(s)", responses.len());
                for response in responses {
                    let modern = ensure_modern_frame(&response);
                    info!("sending response len={}", modern.len());
                    socket.write_all(&modern).await?;
                }
            }
            Err(error) => {
                warn!("character request rejected: {error}");
                if matches!(
                    error,
                    CharacterFlowError::Unauthenticated
                        | CharacterFlowError::MissingTransferTicket(_)
                ) {
                    break;
                }
            }
        }
    }

    Ok(())
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
