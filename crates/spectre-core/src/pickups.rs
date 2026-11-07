//! Live pickup records. Original creation arithmetic with preview OS randomness.
use crate::{recovered, scene::Item};

#[derive(Clone, serde::Serialize)]
pub struct Pickup {
    pub item: Item,
    pub subtype: i16,
    #[serde(skip)]
    pub record: [u8; 128],
}

pub fn create(
    template: &Item,
    mut existing: Vec<[i32; 2]>,
    weights: [i8; 4],
    seed: &mut u32,
) -> crate::Result<Vec<Pickup>> {
    let mut pickups = Vec::new();
    // Independent provisional stream until the shared Mac Random state is recovered.
    let mut remaining = 65536_u32;
    for _ in 0..6 {
        let mut record = [0; 128];
        let created = recovered::create_dump_record(
            &mut record,
            &existing,
            &recovered::DumpCreation {
                extent: 1024,
                kind: 5,
                now: 0,
                weights: [weights[0], weights[1], weights[2]],
                callback: 0,
            },
            || {
                remaining = remaining.checked_sub(1)?;
                *seed ^= *seed << 13;
                *seed ^= *seed >> 17;
                *seed ^= *seed << 5;
                Some(*seed as i16)
            },
        )
        .ok_or("Pickup placement exhausted its host random budget")?;
        if !created {
            break;
        }
        let pose: [i32; 3] = std::array::from_fn(|axis| {
            i32::from_be_bytes(record[axis * 4..axis * 4 + 4].try_into().unwrap())
        });
        let subtype = i16::from_be_bytes(record[0x1c..0x1e].try_into().unwrap());
        let mut item = template.clone();
        item.x = pose[0] as f32 / 65536.;
        item.z = pose[1] as f32 / 65536.;
        item.heading = pose[2] as f32 / 65536.;
        existing.push([pose[0], pose[1]]);
        pickups.push(Pickup {
            item,
            subtype,
            record,
        });
    }
    Ok(pickups)
}

/// Captured original shield creation request; shield-object simulation is pending.
#[derive(Clone, serde::Serialize)]
pub struct ShieldRequest {
    pub position: [i32; 3],
    pub height: i16,
    pub timestamp: i32,
}
