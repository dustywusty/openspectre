use crate::{
    Result,
    binary::{Reader, slice, text},
};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize)]
pub struct Resource {
    pub kind: String,
    pub id: i16,
    pub name: String,
    pub attributes: u8,
    pub offset: usize,
    pub size: usize,
    #[serde(skip)]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Archive {
    pub resources: Vec<Resource>,
}

impl Archive {
    pub fn parse(input: &[u8]) -> Result<Self> {
        if input.len() > 32 * 1024 * 1024 {
            return Err("Resource input exceeds 32 MiB".into());
        }
        // Raw resource forks usually begin with a data offset of 256. MacBinary
        // begins with a zero byte, a filename length, then the filename.
        let data = if input.len() >= 128
            && input[0] == 0
            && (1..=63).contains(&input[1])
            && input[74] == 0
            && input[82] == 0
        {
            let mut r = Reader::new(slice(input, 83, 8)?);
            let data_len = r.u32()? as usize;
            let rsrc_len = r.u32()? as usize;
            let secondary = u16::from_be_bytes([input[120], input[121]]) as usize;
            let start = 128usize
                .checked_add(secondary.div_ceil(128) * 128)
                .and_then(|v| v.checked_add(data_len.div_ceil(128) * 128))
                .ok_or("MacBinary offset overflow")?;
            slice(input, start, rsrc_len)?
        } else {
            input
        };
        let mut h = Reader::new(data);
        let data_offset = h.u32()? as usize;
        let map_offset = h.u32()? as usize;
        let data_len = h.u32()? as usize;
        let map_len = h.u32()? as usize;
        let payload = slice(data, data_offset, data_len)?;
        let map = slice(data, map_offset, map_len)?;
        let mut offsets = Reader::new(slice(map, 24, 4)?);
        let types_offset = offsets.u16()? as usize;
        let names_offset = offsets.u16()? as usize;
        let types = slice(
            map,
            types_offset,
            map.len()
                .checked_sub(types_offset)
                .ok_or("Invalid type offset")?,
        )?;
        let mut tr = Reader::new(types);
        let count = tr.zcount()?;
        if count > 4096 {
            return Err("Too many resource types".into());
        }
        slice(types, 2, count * 8)?;
        let mut resources = Vec::new();
        let mut total_bytes = 0usize;
        let mut seen = BTreeSet::new();
        for _ in 0..count {
            let kind = text(tr.take(4)?);
            let num = tr.zcount()?;
            let refs_offset = tr.u16()? as usize;
            let mut refs = Reader::new(slice(types, refs_offset, num * 12)?);
            for _ in 0..num {
                let id = refs.i16()?;
                if !seen.insert((kind.clone(), id)) {
                    return Err(format!("Duplicate {kind} {id}"));
                }
                let name_offset = refs.u16()?;
                let attrs_offset = refs.u32()?;
                let attributes = (attrs_offset >> 24) as u8;
                let offset = (attrs_offset & 0x00ff_ffff) as usize;
                refs.u32()?;
                let name = if name_offset == 0xffff {
                    String::new()
                } else {
                    let pos = names_offset
                        .checked_add(name_offset as usize)
                        .ok_or("Name offset overflow")?;
                    let len = slice(map, pos, 1)?[0] as usize;
                    text(slice(map, pos + 1, len)?)
                };
                let size = Reader::new(slice(payload, offset, 4)?).u32()? as usize;
                total_bytes = total_bytes
                    .checked_add(size)
                    .ok_or("Resource size overflow")?;
                if total_bytes > 64 * 1024 * 1024 || resources.len() >= 16384 {
                    return Err("Expanded resource limits exceeded".into());
                }
                let bytes = slice(payload, offset + 4, size)?.to_vec();
                resources.push(Resource {
                    kind: kind.clone(),
                    id,
                    name,
                    attributes,
                    offset: data_offset + offset + 4,
                    size,
                    data: bytes,
                });
            }
        }
        Ok(Self { resources })
    }
    pub fn get(&self, kind: &str, id: i16) -> Option<&Resource> {
        self.resources.iter().find(|r| r.kind == kind && r.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        // One named TEXT resource, ID -7, with a three-byte payload.
        let mut b = vec![0u8; 256 + 7 + 53];
        for (i, n) in [256u32, 263, 7, 53].iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&n.to_be_bytes());
        }
        b[256..263].copy_from_slice(&[0, 0, 0, 3, 0x61, 0x62, 0x63]);
        let header: Vec<_> = b[..16].to_vec();
        b[263..279].copy_from_slice(&header);
        b[287..291].copy_from_slice(&[0, 28, 0, 50]);
        b[293..297].copy_from_slice(b"TEXT");
        b[299..301].copy_from_slice(&[0, 10]);
        b[301..303].copy_from_slice(&(-7i16).to_be_bytes());
        b[313..316].copy_from_slice(&[2, b'H', b'i']);
        b
    }
    #[test]
    fn resource_map_and_macbinary_padding() {
        let raw = fixture();
        let archive = Archive::parse(&raw).unwrap();
        let r = archive.get("TEXT", -7).unwrap();
        assert_eq!(r.name, "Hi");
        assert_eq!(r.data, b"abc");
        assert_eq!(r.offset, 260);
        let mut wrapped = vec![0u8; 256];
        wrapped[1] = 4;
        wrapped[2..6].copy_from_slice(b"Test");
        wrapped[83..87].copy_from_slice(&3u32.to_be_bytes());
        wrapped[87..91].copy_from_slice(&(raw.len() as u32).to_be_bytes());
        wrapped[128..131].copy_from_slice(b"xyz");
        wrapped.extend(&raw);
        assert_eq!(
            Archive::parse(&wrapped)
                .unwrap()
                .get("TEXT", -7)
                .unwrap()
                .data,
            b"abc"
        );
        for n in 0..raw.len() {
            assert!(Archive::parse(&raw[..n]).is_err());
        }
    }
    #[test]
    fn hostile_offsets_and_short_inputs_return_errors() {
        for n in 0..256 {
            assert!(Archive::parse(&vec![0xff; n]).is_err());
        }
        let mut b = [0u8; 32];
        b[..4].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(Archive::parse(&b).is_err());
    }
}
