use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::{error::ProtocolError, opcode::CHECKSUM_VALIDATION};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacket {
    pub length: u16,
    pub packet_type: i16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PacketReader {
    cursor: Cursor<Vec<u8>>,
}

impl PacketReader {
    pub fn from_frame(frame: &[u8]) -> Result<RawPacket, ProtocolError> {
        if frame.len() < 6 {
            return Err(ProtocolError::PacketTooShort);
        }

        // Check for 32-bit framed header (u32 LE length, u32 LE checksum with 0x2B4D1A3C).
        if frame.len() >= 10 {
            let u32_len = u32::from_le_bytes(frame[0..4].try_into().unwrap()) as usize;
            if u32_len == frame.len() {
                let checksum = u32::from_le_bytes(frame[frame.len() - 4..].try_into().unwrap());
                let expected = (u32_len as u32) ^ 0x2B4D1A3C_u32;
                if checksum == expected {
                    let packet_type = i16::from_le_bytes(frame[4..6].try_into().unwrap());
                    let payload = frame[6..frame.len() - 4].to_vec();
                    return Ok(RawPacket {
                        length: frame.len() as u16,
                        packet_type,
                        payload,
                    });
                }
            }
        }

        let mut reader = Self::new(frame.to_vec());
        let length = reader.read_u16()?;

        if usize::from(length) != frame.len() {
            return Err(ProtocolError::PacketTooShort);
        }

        let packet_type = reader.read_i16()?;

        reader.seek(u64::from(length.saturating_sub(2)))?;
        let checksum = reader.read_i16()?;
        let expected = (length as i16) ^ CHECKSUM_VALIDATION;
        if checksum != expected {
            return Err(ProtocolError::InvalidChecksum {
                length: usize::from(length),
                expected,
                actual: checksum,
            });
        }

        reader.seek(4)?;
        let payload_len = usize::from(length).saturating_sub(6);
        let payload = reader.read_bytes(payload_len)?;

        Ok(RawPacket {
            length,
            packet_type,
            payload,
        })
    }

    pub fn new(buffer: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(buffer),
        }
    }

    pub fn seek(&mut self, position: u64) -> Result<(), ProtocolError> {
        self.cursor
            .seek(SeekFrom::Start(position))
            .map(|_| ())
            .map_err(|_| ProtocolError::UnexpectedEof)
    }

    pub fn position(&self) -> u64 {
        self.cursor.position()
    }

    pub fn read_u8(&mut self) -> Result<u8, ProtocolError> {
        let mut buf = [0_u8; 1];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| ProtocolError::UnexpectedEof)?;
        Ok(buf[0])
    }

    pub fn read_i16(&mut self) -> Result<i16, ProtocolError> {
        let mut buf = [0_u8; 2];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| ProtocolError::UnexpectedEof)?;
        Ok(i16::from_le_bytes(buf))
    }

    pub fn read_u16(&mut self) -> Result<u16, ProtocolError> {
        let mut buf = [0_u8; 2];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| ProtocolError::UnexpectedEof)?;
        Ok(u16::from_le_bytes(buf))
    }

    pub fn read_u32(&mut self) -> Result<u32, ProtocolError> {
        let mut buf = [0_u8; 4];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| ProtocolError::UnexpectedEof)?;
        Ok(u32::from_le_bytes(buf))
    }

    pub fn read_u64(&mut self) -> Result<u64, ProtocolError> {
        let mut buf = [0_u8; 8];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| ProtocolError::UnexpectedEof)?;
        Ok(u64::from_le_bytes(buf))
    }

    pub fn read_i32(&mut self) -> Result<i32, ProtocolError> {
        let mut buf = [0_u8; 4];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| ProtocolError::UnexpectedEof)?;
        Ok(i32::from_le_bytes(buf))
    }

    pub fn read_f32(&mut self) -> Result<f32, ProtocolError> {
        let mut buf = [0_u8; 4];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| ProtocolError::UnexpectedEof)?;
        Ok(f32::from_le_bytes(buf))
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, ProtocolError> {
        let mut buf = vec![0_u8; len];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| ProtocolError::UnexpectedEof)?;
        Ok(buf)
    }

    pub fn remaining_len(&self) -> usize {
        self.cursor
            .get_ref()
            .len()
            .saturating_sub(self.cursor.position() as usize)
    }

    pub fn read_string(&mut self) -> Result<String, ProtocolError> {
        let len = self.read_u8()? as usize;
        let bytes = self.read_bytes(len)?;
        let _terminator = self.read_u8()?;
        Ok(String::from_utf8_lossy(&bytes).trim().to_string())
    }

    /// Read a variable-length text field: `[u16 byte-length][UTF-8 bytes]` with no
    /// terminator, bounded at 512 bytes. A zero length yields an empty value; an
    /// over-bound length is a protocol error rather than a truncation, so a
    /// misframed stream cannot be silently absorbed.
    pub fn read_modern_utf8_string(&mut self) -> Result<String, ProtocolError> {
        let len = self.read_u16()? as usize;
        if len > 512 {
            return Err(ProtocolError::InvalidTextLength(len));
        }
        if len == 0 {
            return Ok(String::new());
        }
        let bytes = self.read_bytes(len)?;
        match String::from_utf8(bytes.clone()) {
            Ok(value) => Ok(value.trim().to_string()),
            Err(_) => Ok(String::from_utf8_lossy(&bytes).trim().to_string()),
        }
    }

    /// Read a length-prefixed UTF-16LE string: `[u8 code-unit count][units...][u16 0]`.
    /// The wide analogue of `read_string`; the count is in UTF-16 code units.
    pub fn read_wide_string(&mut self) -> Result<String, ProtocolError> {
        let len = self.read_u8()? as usize;
        let mut units = Vec::with_capacity(len);
        for _ in 0..len {
            units.push(self.read_u16()?);
        }
        let _terminator = self.read_u16()?;
        Ok(String::from_utf16_lossy(&units).trim().to_string())
    }

    pub fn read_zstring(&mut self) -> Result<String, ProtocolError> {
        let mut out = Vec::new();
        loop {
            let byte = self.read_u8()?;
            if byte == 0 {
                break;
            }
            out.push(byte);
        }
        Ok(String::from_utf8_lossy(&out).to_string())
    }
}
