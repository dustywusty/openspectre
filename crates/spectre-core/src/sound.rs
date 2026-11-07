//! Standard 8-bit sampled Macintosh snd resources. Compressed/extended headers
//! are rejected explicitly. Format follows Apple's Inside Macintosh: Sound.
use crate::{
    Result,
    binary::{Reader, slice},
};
pub struct Sound {
    pub rate: u32,
    pub samples: Vec<u8>,
}
impl Sound {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        match r.u16()? {
            1 => {
                let n = r.u16()? as usize;
                r.take(n * 6)?;
            }
            2 => {
                r.u16()?;
            }
            _ => return Err("Unsupported sound resource format".into()),
        }
        let count = r.u16()?;
        let mut header = None;
        for _ in 0..count {
            let command = r.u16()?;
            r.u16()?;
            let offset = r.u32()? as usize;
            if matches!(command, 0x8050 | 0x8051) {
                if header.replace(offset).is_some() {
                    return Err("Multiple sample commands are unsupported".into());
                }
            } else {
                return Err(format!("Unsupported sound command {command:#x}"));
            }
        }
        let offset = header.ok_or("Missing sample command")?;
        if offset < r.pos {
            return Err("Sound header overlaps commands".into());
        }
        let mut h = Reader::new(slice(data, offset, 22)?);
        if h.u32()? != 0 {
            return Err("External sound sample pointer is unsupported".into());
        }
        let length = h.u32()? as usize;
        let rate_fixed = h.u32()?;
        h.u32()?;
        h.u32()?; // loop range: effects play once
        if h.take(1)?[0] != 0 {
            return Err("Compressed or extended sound header is unsupported".into());
        }
        h.take(1)?; // base frequency is metadata for note commands, not bufferCmd
        let rate = ((rate_fixed as u64 + 32768) >> 16) as u32;
        if !(1000..=192000).contains(&rate) {
            return Err("Unsupported sound sample rate".into());
        }
        Ok(Self {
            rate,
            samples: slice(data, offset + 22, length)?.to_vec(),
        })
    }
    pub fn wav(&self) -> Vec<u8> {
        let length = self.samples.len() as u32;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + length + (length & 1)).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&8_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&self.samples);
        if length & 1 != 0 {
            out.push(0);
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_pcm_and_truncated_payload() {
        let mut bytes = vec![0, 2, 0, 0, 0, 1, 0x80, 0x51, 0, 0, 0, 0, 0, 14];
        bytes.extend_from_slice(&[
            0, 0, 0, 0, 0, 0, 0, 3, 0x2b, 0x11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 60,
        ]);
        bytes.extend_from_slice(&[0, 128, 255]);
        let sound = Sound::parse(&bytes).unwrap();
        assert_eq!(sound.rate, 11025);
        assert_eq!(sound.samples, [0, 128, 255]);
        let wav = sound.wav();
        assert_eq!(&wav[44..47], &[0, 128, 255]);
        assert_eq!(wav.len(), 48);
        for end in 0..bytes.len() {
            assert!(Sound::parse(&bytes[..end]).is_err());
        }
        bytes[34] = 0xfe;
        assert!(Sound::parse(&bytes).is_err());
    }
}
