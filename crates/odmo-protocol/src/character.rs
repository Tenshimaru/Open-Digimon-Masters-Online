use odmo_types::{AccountId, CharacterSummary};

use crate::{
    error::ProtocolError,
    opcode::character,
    reader::{PacketReader, RawPacket},
    writer::PacketWriter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacterRequest {
    Connection {
        kind: u8,
    },
    KeepConnection,
    RequestCharacters {
        account_id: AccountId,
    },
    CreateCharacter {
        slot: u8,
        tamer_model: i32,
        tamer_name: String,
        partner_model: i32,
        partner_name: String,
    },
    DeleteCharacter {
        slot: u8,
        validation: String,
    },
    GetCharacterPosition {
        slot: u8,
    },
    ConnectGameServer,
    CheckNameDuplicity {
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteCharacterResult {
    Error = 0,
    Deleted = 1,
    ValidationFail = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterCreationFailure {
    Generic = 1,
    ConflictingTamerName = 9,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterConnectionPacket {
    pub handshake: i16,
}

impl CharacterConnectionPacket {
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
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);
        packet[8..12].copy_from_slice(&ts.to_le_bytes());
        let checksum: u32 = 16u32 ^ 0x2B4D1A3Cu32;
        packet[12..16].copy_from_slice(&checksum.to_le_bytes());
        packet
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterListPacket {
    pub characters: Vec<CharacterSummary>,
}

impl CharacterListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(character::CHARACTER_LIST);
        // Character list header: [count: u8][flags1: u8][flags2: u16]
        writer.write_u8(self.characters.len() as u8);
        writer.write_u8(0);
        writer.write_u16(0);

        for character in &self.characters {
            // [slot: u32][id: u32][map_id: i16][model: i32][level: u8][name: modern string]
            writer.write_u32(character.slot as u32);
            writer.write_u32(character.id as u32);
            writer.write_i16(character.map_id);
            writer.write_i32(character.model);
            writer.write_u8(character.level);
            writer.write_modern_utf8_string(&character.name);

            // Equipment/items: exactly 0x450 (1104) bytes
            writer.write_zeroes(1104);

            // [partner_model: i32][partner_level: u8][partner_name: modern string][partner_size: i16]
            writer.write_i32(character.partner_model);
            writer.write_u8(character.partner_level);
            writer.write_modern_utf8_string(&character.partner_name);
            writer.write_i16(character.partner_size);

            // Trailer: [u32][u16][u8]
            writer.write_u32(0);
            writer.write_u16(0);
            writer.write_u8(0);
        }

        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectGameServerInfoPacket {
    pub address: String,
    pub port: u16,
    pub map_id: i16,
}

impl ConnectGameServerInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(character::CONNECT_GAME_SERVER_INFO);
        // Payload layout: [address: u16 LE len + UTF-8 bytes][port: i32][map_id: i32]
        writer.write_modern_utf8_string(&self.address);
        writer.write_i32(self.port as i32);
        writer.write_i32(self.map_id as i32);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectGameServerPacket;

impl ConnectGameServerPacket {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(character::CONNECT_GAME_SERVER).finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableNamePacket {
    pub available: bool,
}

impl AvailableNamePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(character::CHECK_NAME_DUPLICITY);
        writer.write_i32(self.available as i32);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterDeletedPacket {
    pub result: DeleteCharacterResult,
}

impl CharacterDeletedPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(character::CHARACTER_DELETED);
        writer.write_i32(self.result as i32);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreationFailedPacket {
    pub result: CharacterCreationFailure,
}

impl CharacterCreationFailedPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(character::CHARACTER_CREATION_FAILED);
        writer.write_u32(self.result as u32);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreatedPacket {
    pub character: CharacterSummary,
    pub handshake: i16,
}

impl CharacterCreatedPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(character::CHARACTER_CREATED);
        writer.write_i16(self.handshake);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.write_i16(self.handshake);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.write_u8(self.character.slot);
        writer.write_i16(self.character.map_id);
        writer.write_i32(self.character.model);
        writer.write_u8(1);
        writer.write_string(&self.character.name);

        for _ in 0..18 {
            writer.write_bytes(&[0; 60]);
        }

        writer.write_i32(self.character.partner_model);
        writer.write_u8(1);
        writer.write_string(&self.character.partner_name);
        writer.write_bytes(&[0; 8]);
        writer.finalize()
    }
}

impl TryFrom<RawPacket> for CharacterRequest {
    type Error = ProtocolError;

    fn try_from(packet: RawPacket) -> Result<Self, Self::Error> {
        let mut reader = PacketReader::new(packet.payload);
        match packet.packet_type {
            character::CONNECTION => Ok(Self::Connection {
                kind: reader.read_u8()?,
            }),
            character::KEEP_CONNECTION => Ok(Self::KeepConnection),
            character::REQUEST_CHARACTERS => {
                // Client AccessCode packet payload: [XOR(4)][accountIdx(4)][accessCode(4)]
                // AccessCode payload: [XOR(4)][accountIdx(4)][accessCode(4)]
                reader.seek(4)?;
                Ok(Self::RequestCharacters {
                    account_id: reader.read_u32()? as AccountId,
                })
            }
            character::CREATE_CHARACTER => Ok(parse_create_character(&mut reader)?),
            character::DELETE_CHARACTER => {
                // The list slot rides as a dword, then the second-password text in modern form.
                let slot = reader.read_u32()? as u8;
                Ok(Self::DeleteCharacter {
                    slot,
                    validation: reader.read_modern_utf8_string()?,
                })
            }
            character::GET_CHARACTER_POSITION => {
                let slot = if reader.remaining_len() >= 4 {
                    reader.read_i32()? as u8
                } else {
                    reader.read_u8()?
                };
                Ok(Self::GetCharacterPosition { slot })
            }
            character::CONNECT_GAME_SERVER => Ok(Self::ConnectGameServer),
            character::CHECK_NAME_DUPLICITY => Ok(Self::CheckNameDuplicity {
                name: reader.read_string()?,
            }),
            other => Err(ProtocolError::InvalidCharacterPacketType(other)),
        }
    }
}

fn parse_create_character(reader: &mut PacketReader) -> Result<CharacterRequest, ProtocolError> {
    let slot = reader.read_u8()?;
    let tamer_model = reader.read_i32()?;
    let tamer_name = read_fixed_ascii(reader, 33)?;
    let partner_model = reader.read_i32()?;
    let partner_name = read_fixed_ascii(reader, 33)?;

    Ok(CharacterRequest::CreateCharacter {
        slot,
        tamer_model,
        tamer_name,
        partner_model,
        partner_name,
    })
}

fn read_fixed_ascii(reader: &mut PacketReader, len: usize) -> Result<String, ProtocolError> {
    let bytes = reader.read_bytes(len)?;
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    Ok(String::from_utf8_lossy(&bytes[..end]).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::PacketReader;
    use odmo_types::{DEFAULT_PARTNER_MODEL_ID, DEFAULT_TAMER_MODEL_ID};

    #[test]
    fn parse_request_characters_reads_account_id_from_offset_4() {
        // Client sends: newp(1706); push(XOR); push(accountIdx); push(accessCode); endp();
        // Payload = [XOR(4)][accountIdx(4)][accessCode(4)] = 12 bytes
        // Skip XOR value; read accountIdx at offset 4 in payload
        let mut payload = Vec::new();
        payload.extend_from_slice(&(0xDEAD_u32).to_le_bytes()); // XOR value (skipped)
        payload.extend_from_slice(&(1_u32).to_le_bytes()); // account_idx
        payload.extend_from_slice(&(0xBEEF_u32).to_le_bytes()); // access_code

        let request = CharacterRequest::try_from(RawPacket {
            length: 0,
            packet_type: character::REQUEST_CHARACTERS,
            payload,
        })
        .expect("request should parse");

        assert_eq!(
            request,
            CharacterRequest::RequestCharacters { account_id: 1 }
        );
    }

    #[test]
    fn parse_delete_character_reads_dword_slot_and_modern_text() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&(4u16).to_le_bytes());
        payload.extend_from_slice(b"pass");

        let request = CharacterRequest::try_from(RawPacket {
            length: 0,
            packet_type: character::DELETE_CHARACTER,
            payload,
        })
        .expect("request should parse");

        assert_eq!(
            request,
            CharacterRequest::DeleteCharacter {
                slot: 2,
                validation: "pass".to_string(),
            }
        );
    }

    #[test]
    fn character_list_packet_uses_expected_opcode() {
        let packet = CharacterListPacket {
            characters: vec![CharacterSummary {
                id: 1,
                account_id: 1,
                slot: 0,
                name: "Admin".to_string(),
                partner_name: "Agumon".to_string(),
                model: DEFAULT_TAMER_MODEL_ID,
                partner_model: DEFAULT_PARTNER_MODEL_ID,
                ..CharacterSummary::default()
            }],
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, character::CHARACTER_LIST);
    }

    #[test]
    fn parse_create_character_uses_fixed_name_fields() {
        let mut payload = Vec::new();
        payload.push(1);
        payload.extend_from_slice(&DEFAULT_TAMER_MODEL_ID.to_le_bytes());
        let mut tamer = [0_u8; 33];
        tamer[..5].copy_from_slice(b"Admin");
        payload.extend_from_slice(&tamer);
        payload.extend_from_slice(&31001_i32.to_le_bytes());
        let mut partner = [0_u8; 33];
        partner[..6].copy_from_slice(b"Agumon");
        payload.extend_from_slice(&partner);

        let request = CharacterRequest::try_from(RawPacket {
            length: 0,
            packet_type: character::CREATE_CHARACTER,
            payload,
        })
        .expect("request should parse");

        assert_eq!(
            request,
            CharacterRequest::CreateCharacter {
                slot: 1,
                tamer_model: DEFAULT_TAMER_MODEL_ID,
                tamer_name: "Admin".to_string(),
                partner_model: 31001,
                partner_name: "Agumon".to_string(),
            }
        );
    }
}
