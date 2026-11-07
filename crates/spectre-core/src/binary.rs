use crate::Result;

pub fn slice(data: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset.checked_add(len).ok_or("Offset overflow")?;
    data.get(offset..end)
        .ok_or_else(|| format!("Truncated data at {offset:#x}: need {len} bytes"))
}

pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}
impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    pub fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let result = slice(self.data, self.pos, len)?;
        self.pos += len;
        Ok(result)
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    /// ResEdit ZCNT stores count minus one; 0xffff represents an empty list.
    pub fn zcount(&mut self) -> Result<usize> {
        Ok(self.u16()?.wrapping_add(1) as usize)
    }
    pub fn finish(&self) -> Result<()> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(format!(
                "{} unparsed bytes at {:#x}",
                self.data.len() - self.pos,
                self.pos
            ))
        }
    }
}

// Preserve non-ASCII Mac Roman bytes explicitly until text conversion is needed.
// ASCII resource names remain exact; escaped bytes are reversible.
pub fn text(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (32..127).contains(&b) {
                (b as char).to_string()
            } else {
                format!("\\x{b:02x}")
            }
        })
        .collect()
}
