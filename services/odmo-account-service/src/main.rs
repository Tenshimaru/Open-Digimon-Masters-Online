use anyhow::Context;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, warn};

use odmo_application::account::{
    AccountApplication, AccountFlowError, AccountServiceConfig, SessionFactory,
};
use odmo_protocol::{AccountRequest, PacketReader};
use odmo_types::CharacterServerTarget;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .compact()
        .init();

    let bind = std::env::var("ODMO_ACCOUNT_BIND").unwrap_or_else(|_| "127.0.0.1:7029".to_string());
    let character_host =
        std::env::var("ODMO_CHARACTER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let character_port = std::env::var("ODMO_CHARACTER_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(7050);
    let portal_state_dir = std::env::var("ODMO_PORTAL_STATE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("odmo-portal"));

    let backend = std::sync::Arc::new(odmo_persistence::initialize_backend().await?);
    let repository = backend.account_repository();

    let app = AccountApplication::new(
        AccountServiceConfig {
            character_server: CharacterServerTarget {
                address: character_host,
                port: character_port,
            },
            portal_state_dir,
            use_resource_hash: std::env::var("ODMO_USE_RESOURCE_HASH")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        },
        repository,
    );
    let session_factory = SessionFactory::new();
    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind account service on {bind}"))?;

    info!("account service listening on {bind}");

    loop {
        let (socket, address) = listener.accept().await?;
        info!("accepted account connection from {address}");

        let app = app.clone();
        let mut session = session_factory.create();

        tokio::spawn(async move {
            if let Err(error) = serve_client(socket, &app, &mut session).await {
                error!("client session ended with error: {error:#}");
            }
        });
    }
}

async fn serve_client(
    mut socket: TcpStream,
    app: &AccountApplication,
    session: &mut odmo_application::account::AccountSession,
) -> anyhow::Result<()> {
    // Send initial server handshake (opcode -1, 22-byte framed header).
    let mut initial_handshake = vec![0u8; 22];
    initial_handshake[0..4].copy_from_slice(&22u32.to_le_bytes());
    initial_handshake[4..6].copy_from_slice(&(-1i16).to_le_bytes());
    initial_handshake[6..8].copy_from_slice(&session.handshake_seed.to_le_bytes());
    let checksum: u32 = 22u32 ^ 0x2B4D1A3Cu32;
    initial_handshake[18..22].copy_from_slice(&checksum.to_le_bytes());

    info!(
        "sending initial account handshake (22 bytes, seed={}): {:02X?}",
        session.handshake_seed,
        &initial_handshake[..]
    );
    socket.write_all(&initial_handshake).await?;
    socket.flush().await.ok();
    info!("account handshake sent, waiting for client response...");

    loop {
        let frame = match read_frame(&mut socket).await {
            Ok(frame) => frame,
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        };

        let raw = match PacketReader::from_frame(&frame) {
            Ok(packet) => {
                info!(
                    "received account packet opcode={} payload_len={}",
                    packet.packet_type,
                    packet.payload.len()
                );
                packet
            }
            Err(error) => {
                warn!("dropping invalid packet: {error}");
                continue;
            }
        };

        let request = match AccountRequest::try_from(raw) {
            Ok(request) => {
                info!("parsed account request: {request:?}");
                request
            }
            Err(error) => {
                warn!("unsupported account request: {error}");
                continue;
            }
        };

        match app.handle_request(session, request) {
            Ok(responses) => {
                for response in responses {
                    let modern = ensure_modern_frame(&response);
                    info!("sending response len={}", modern.len());
                    socket.write_all(&modern).await?;
                }
            }
            Err(error) => {
                if let Some(response) = AccountApplication::failure_packet(&error) {
                    let modern = ensure_modern_frame(&response);
                    socket.write_all(&modern).await?;
                } else {
                    warn!("request rejected: {error}");
                    if matches!(error, AccountFlowError::Unauthenticated) {
                        break;
                    }
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
    // 32-bit length headers carry length in bytes 0..4 (header[2..4] == [0, 0] for <= 65535 bytes);
    // 16-bit compact headers store length in bytes 0..2 and opcode in bytes 2..4.
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
