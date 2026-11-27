use odmo_types::{AccountId, CharacterServerTarget, ServerDescriptor};

use crate::{
    error::ProtocolError,
    opcode::account,
    reader::{PacketReader, RawPacket},
    writer::PacketWriter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountRequest {
    Connection {
        kind: u8,
    },
    KeepConnection,
    Login(LoginPayload),
    SecondaryPasswordRegister {
        password: String,
    },
    SecondaryPasswordCheck {
        check_mode: SecondaryPasswordCheck,
        password: Option<String>,
    },
    SecondaryPasswordChange {
        current_password: String,
        new_password: String,
    },
    LoadServerList,
    ConnectCharacterServer {
        server_id: i32,
    },
    ResourcesHash {
        client_hash: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
    pub cpu: String,
    pub gpu: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginResponse {
    UserNotFound = 1,
    IncorrectPassword = 2,
    AccountBlocked = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryPasswordScreen {
    Hide = 1,
    RequestInput = 2,
    RequestSetup = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryPasswordCheck {
    Check = 2,
    DontCheck = 3,
    CorrectOrSkipped = 0,
    Incorrect = 20052,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryPasswordChange {
    Changed = 0,
    IncorrectCurrentPassword = 20052,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionPacket {
    pub handshake: i16,
    pub handshake_timestamp: u32,
}

impl ConnectionPacket {
    /// Handshake confirmation response (opcode -2 / CONNECTION_RESPONSE).
    /// Wire framing: 16 bytes total
    /// [0..3]: Length = 16 (u32 LE: 10 00 00 00)
    /// [4..5]: Opcode = -2 (i16 LE: FE FF)
    /// [6..7]: Handshake (u16 LE = seed ^ 0x7E41)
    /// [8..11]: Handshake timestamp (u32 LE)
    /// [12..15]: Checksum = 16 ^ 0x2B4D1A3C = 0x2B4D1A2C (u32 LE: 2C 1A 4D 2B)
    pub fn encode(&self) -> Vec<u8> {
        let mut packet = vec![0u8; 16];
        packet[0..4].copy_from_slice(&16u32.to_le_bytes());
        packet[4..6].copy_from_slice(&(-2i16).to_le_bytes());
        packet[6..8].copy_from_slice(&self.handshake.to_le_bytes());
        packet[8..12].copy_from_slice(&self.handshake_timestamp.to_le_bytes());
        let checksum: u32 = 16u32 ^ 0x2B4D1A3Cu32;
        packet[12..16].copy_from_slice(&checksum.to_le_bytes());
        packet
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginRequestAnswerPacket {
    Failed(LoginResponse),
    Success(SecondaryPasswordScreen),
}

impl LoginRequestAnswerPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(account::LOGIN_REQUEST);
        match self {
            Self::Failed(reason) => {
                writer.write_u8(*reason as u8);
                writer.write_u8(39);
                writer.write_u8(0);
                writer.write_u8(0);
                writer.write_u8(0);
            }
            Self::Success(screen) => {
                writer.write_i32(0);
                writer.write_u8(*screen as u8);
            }
        }
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginRequestBannedAnswerPacket {
    pub remaining_time_in_seconds: u32,
    pub reason: String,
}

impl LoginRequestBannedAnswerPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(3308);
        writer.write_u32(self.remaining_time_in_seconds);
        writer.write_modern_utf8_string(&self.reason);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerListPacket {
    pub servers: Vec<ServerDescriptor>,
}

impl ServerListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(account::LOAD_SERVER_LIST);
        writer.write_u8(self.servers.len() as u8);
        for server in &self.servers {
            writer.write_i32(server.id as i32);
            writer.write_modern_utf8_string(&server.name);
            writer.write_u8(server.maintenance as u8);
            writer.write_u8(server.overloaded as u8);
            writer.write_u8(server.character_count);
            writer.write_u8(server.is_new as u8);
            // Trailing slot capacity fields: [max_tamer_slot: u8][open_tamer_slot: u8]
            writer.write_u8(4);
            writer.write_u8(4);
        }
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryPasswordCheckResultPacket {
    pub result: SecondaryPasswordCheck,
}

impl SecondaryPasswordCheckResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(9804);
        writer.write_i32(self.result as i32);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryPasswordChangeResultPacket {
    pub result: SecondaryPasswordChange,
}

impl SecondaryPasswordChangeResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(9806);
        writer.write_i32(self.result as i32);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcesHashPacket {
    pub hash_hex: String,
}

impl ResourcesHashPacket {
    pub fn encode(&self) -> Vec<u8> {
        let hash_bytes = decode_hex(&self.hash_hex);
        let mut writer = PacketWriter::new(10003);
        writer.write_i16((hash_bytes.len()) as i16);
        writer.write_bytes(&hash_bytes);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectCharacterServerPacket {
    pub account_id: AccountId,
    /// Per-login secret the client echoes to the character and game services.
    pub access_code: u32,
    pub target: CharacterServerTarget,
}

impl ConnectCharacterServerPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(901);
        // Payload layout: [account_idx: u32][access_code: u32][ip: u16 LE len + UTF-8 bytes][port: u32]
        writer.write_u32(self.account_id as u32); // account_idx
        writer.write_u32(self.access_code); // access_code
        writer.write_modern_utf8_string(&self.target.address); // ip address
        writer.write_u32(self.target.port as u32); // port
        writer.finalize()
    }
}

impl TryFrom<RawPacket> for AccountRequest {
    type Error = ProtocolError;

    fn try_from(packet: RawPacket) -> Result<Self, Self::Error> {
        let mut reader = PacketReader::new(packet.payload);
        match packet.packet_type {
            account::CONNECTION => Ok(Self::Connection {
                kind: reader.read_u8()?,
            }),
            account::KEEP_CONNECTION => Ok(Self::KeepConnection),
            account::LOGIN_REQUEST => Ok(Self::Login(parse_login_payload(&mut reader)?)),
            9801 => Ok(Self::SecondaryPasswordRegister {
                password: reader.read_modern_utf8_string()?,
            }),
            9804 => Ok(parse_secondary_password_check(&mut reader)?),
            9806 => Ok(Self::SecondaryPasswordChange {
                current_password: reader.read_modern_utf8_string()?,
                new_password: reader.read_modern_utf8_string()?,
            }),
            account::LOAD_SERVER_LIST => Ok(Self::LoadServerList),
            account::CONNECT_CHARACTER_SERVER => Ok(Self::ConnectCharacterServer {
                server_id: reader.read_i32()?,
            }),
            10003 => Ok(Self::ResourcesHash {
                client_hash: reader.read_bytes(reader.remaining_len())?,
            }),
            other => Err(ProtocolError::InvalidAccountPacketType(other)),
        }
    }
}

fn parse_login_payload(reader: &mut PacketReader) -> Result<LoginPayload, ProtocolError> {
    let start_pos = reader.position();

    // Primary UTF-8 login payload layout:
    // 1. net_version: u32
    // 2. user_type: modern_utf8_string
    // 3. password: modern_utf8_string
    // 4. username: modern_utf8_string
    // 5. cpu: modern_utf8_string
    // 6. gpu: modern_utf8_string
    // 7. ram: u32
    // 8. os: modern_utf8_string
    // 9. mac: modern_utf8_string
    let try_modern = |reader: &mut PacketReader| -> Result<LoginPayload, ProtocolError> {
        let _net_version = reader.read_u32()?;
        let _user_type = reader.read_modern_utf8_string()?;
        let first = reader.read_modern_utf8_string()?;
        let second = reader.read_modern_utf8_string()?;
        let cpu = reader.read_modern_utf8_string().unwrap_or_default();
        let gpu = reader.read_modern_utf8_string().unwrap_or_default();
        let _ram = reader.read_u32().unwrap_or_default();
        let _os = reader.read_modern_utf8_string().unwrap_or_default();
        let _mac = reader.read_modern_utf8_string().unwrap_or_default();

        // Support both orders: (username, password) or (password_md5, username)
        let (username, password) = if first.len() == 32
            && second.len() != 32
            && first.chars().all(|c| c.is_ascii_hexdigit())
        {
            (second, first)
        } else {
            (first, second)
        };

        if !username.is_empty() {
            Ok(LoginPayload {
                username,
                password,
                cpu,
                gpu,
            })
        } else {
            Err(ProtocolError::InvalidTextLength(0))
        }
    };

    let mut modern_reader = reader.clone();
    if let Ok(payload) = try_modern(&mut modern_reader) {
        *reader = modern_reader;
        return Ok(payload);
    }

    // Fallback to compact string layout: [u4 net_version][text user_type][text username][u1 dummy][text password]
    let _ = reader.seek(start_pos);
    let _net_version = reader.read_u32()?;
    let _user_type = reader.read_string()?;
    let username = reader.read_string()?;
    let _dummy = reader.read_u8()?;
    let password = reader.read_string()?;

    Ok(LoginPayload {
        username,
        password,
        cpu: String::new(),
        gpu: String::new(),
    })
}

fn parse_secondary_password_check(
    reader: &mut PacketReader,
) -> Result<AccountRequest, ProtocolError> {
    let raw_mode = reader.read_i16()?;
    let check_mode = match raw_mode {
        2 => SecondaryPasswordCheck::Check,
        3 => SecondaryPasswordCheck::DontCheck,
        _ => return Err(ProtocolError::InvalidAccountPacketType(raw_mode)),
    };

    let password = if check_mode == SecondaryPasswordCheck::Check {
        Some(reader.read_modern_utf8_string()?)
    } else {
        None
    };

    Ok(AccountRequest::SecondaryPasswordCheck {
        check_mode,
        password,
    })
}

fn decode_hex(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks(2)
        .filter_map(|pair| {
            let hi = *pair.first()?;
            let lo = *pair.get(1)?;
            let hex_pair = [hi, lo];
            let text = std::str::from_utf8(&hex_pair).ok()?;
            u8::from_str_radix(text, 16).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::PacketReader;

    #[test]
    fn connection_packet_uses_legacy_framing() {
        let packet = ConnectionPacket {
            handshake: 123,
            handshake_timestamp: 456,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, -2);
    }

    #[test]
    fn login_success_matches_legacy_shape() {
        let packet = LoginRequestAnswerPacket::Success(SecondaryPasswordScreen::Hide).encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");

        assert_eq!(raw.packet_type, account::LOGIN_REQUEST);
        assert_eq!(raw.payload, vec![0, 0, 0, 0, 1]);
    }

    #[test]
    fn parse_login_payload_uses_terminated_text_fields() {
        // Client sends: [u4 net_version][text user_type][text username][u1 dummy][text password]
        // Each text field is [u8 byte count][bytes][u8 0].
        let mut payload = Vec::new();
        payload.extend_from_slice(&1234_u32.to_le_bytes()); // net_version
        payload.push(4);
        payload.extend_from_slice(b"test"); // user_type
        payload.push(0);
        payload.push(4);
        payload.extend_from_slice(b"demo"); // username
        payload.push(0);
        payload.push(0); // dummy
        payload.push(4);
        payload.extend_from_slice(b"pass"); // password
        payload.push(0);

        let raw = RawPacket {
            length: 0,
            packet_type: account::LOGIN_REQUEST,
            payload,
        };

        let request = AccountRequest::try_from(raw).expect("request should parse");
        match request {
            AccountRequest::Login(login) => {
                assert_eq!(login.username, "demo");
                assert_eq!(login.password, "pass");
            }
            other => panic!("unexpected request: {other:?}"),
        }
    }

    #[test]
    fn parse_secondary_password_check_packet() {
        let raw = RawPacket {
            length: 0,
            packet_type: 9804,
            payload: {
                let mut payload = Vec::new();
                payload.extend_from_slice(&(2_i16).to_le_bytes());
                payload.extend_from_slice(&(4_u16).to_le_bytes());
                payload.extend_from_slice(b"4321");
                payload
            },
        };

        let request = AccountRequest::try_from(raw).expect("request should parse");
        assert_eq!(
            request,
            AccountRequest::SecondaryPasswordCheck {
                check_mode: SecondaryPasswordCheck::Check,
                password: Some("4321".to_string()),
            }
        );
    }
}
