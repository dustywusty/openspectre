//! Original radar glyph extraction and the owned PICT 1000 atlas layout.

/// CODE 6 +29c..+320 samples columns before rows, retaining signed XY offsets.
pub fn glyphs_from_bitmap(bitmap: &[u8]) -> Option<Vec<Vec<[i8; 2]>>> {
    if bitmap.len() != 164 * 5 {
        return None;
    }
    Some(
        (0..41)
            .map(|glyph| {
                let mut points = Vec::new();
                for x in 0..4 {
                    for y in 0..5 {
                        if bitmap[y * 164 + glyph * 4 + x] != 0 {
                            points.push([x as i8 - 1, y as i8 - 2]);
                        }
                    }
                }
                points
            })
            .collect(),
    )
}

/// Decode only the monochrome PackBitsRect layout used by the owned PICT 1000.
/// Other PICT encodings are rejected rather than interpreted as this atlas.
pub fn glyphs_from_pict(bytes: &[u8]) -> Option<Vec<Vec<[i8; 2]>>> {
    let header = bytes.get(..52)?;
    let word = |i| u16::from_be_bytes([header[i], header[i + 1]]);
    if usize::from(word(0)) != bytes.len()
        || header[10..13] != [0x11, 1, 1]
        || word(13) != 10
        || header[23] != 0x98
        || word(24) != 22
        || word(50) != 0
    {
        return None;
    }
    let rect = |i| [word(i), word(i + 2), word(i + 4), word(i + 6)];
    let frame = rect(2);
    let bounds = rect(26);
    if rect(15) != frame
        || rect(34) != frame
        || rect(42) != frame
        || frame[2].checked_sub(frame[0])? != 5
        || frame[3].checked_sub(frame[1])? != 164
        || bounds[0] != frame[0]
        || bounds[2] != frame[2]
        || bounds[3].checked_sub(bounds[1])? != 176
    {
        return None;
    }
    let start = usize::from(frame[1].checked_sub(bounds[1])?);
    if start + 164 > 176 {
        return None;
    }
    let mut bitmap = Vec::with_capacity(820);
    let mut cursor = 52;
    for _ in 0..5 {
        let length = usize::from(*bytes.get(cursor)?);
        cursor += 1;
        let packed = bytes.get(cursor..cursor.checked_add(length)?)?;
        cursor += length;
        let mut row = Vec::with_capacity(22);
        let mut i = 0;
        while i < packed.len() {
            let control = packed[i];
            i += 1;
            match control {
                0..=127 => {
                    let count = usize::from(control) + 1;
                    row.extend_from_slice(packed.get(i..i.checked_add(count)?)?);
                    i += count;
                }
                128 => (),
                _ => {
                    let value = *packed.get(i)?;
                    i += 1;
                    row.extend(std::iter::repeat_n(value, 257 - usize::from(control)));
                }
            }
            if row.len() > 22 {
                return None;
            }
        }
        if row.len() != 22 {
            return None;
        }
        bitmap.extend((start..start + 164).map(|x| u8::from(row[x / 8] & (128 >> (x % 8)) != 0)));
    }
    if bytes.get(cursor..) != Some(&[0xff]) {
        return None;
    }
    glyphs_from_bitmap(&bitmap)
}

/// CODE 6 +1e86..+1f44. Only types dispatched to this enemy branch are supported.
/// `final_frame` is the result of the original type-31 animation gate (+197e).
/// Earlier participant visibility gates remain the caller's responsibility.
pub fn enemy_glyph(kind: i8, view: i16, color: u8, final_frame: bool) -> Option<i16> {
    if ![7, 8, 13, 20, 22, 26, 27, 31, 35, 45].contains(&kind) || view >= 2 || color == 13 {
        return None;
    }
    match kind {
        22 => Some(18),
        13 => Some(12),
        20 => Some(17),
        31 if !final_frame => None,
        _ => Some(39),
    }
}

/// CODE 6 +26d0..+271e: retired kind-10 radar marker.
/// Admission uses the kind's radar color; the glyph uses its body color.
pub fn retired_glyph(view: i16, body_color: u8, radar_color: u8) -> Option<(i16, u8)> {
    (view < 2 && radar_color != 13).then_some((39, body_color))
}

#[derive(Debug, serde::Serialize)]
pub struct Scale {
    pub packed_extent: i32,
    pub distance_limit: i32,
    pub outer_distance: i32,
    pub threshold: i32,
    pub scale_base: i32,
    pub far_scale: i32,
    pub falloff: i32,
}

/// CODE 33 +284..+30c, including original signed long multiplication/division.
/// A zero divisor is rejected at the host boundary.
pub fn scale_for_arena(extent: i16) -> Option<Scale> {
    let packed_extent = i32::from(extent.wrapping_add(80).wrapping_shl(3));
    let distance_limit = packed_extent.wrapping_mul(packed_extent).wrapping_mul(2) / 65536;
    let outer_distance = 6400 * 6400 / 65536;
    let threshold = outer_distance / 4;
    let scale_base: i32 = 32767;
    let far_scale = 18100;
    let falloff = ((scale_base - far_scale) << 8_i32).checked_div(distance_limit - threshold)?;
    Some(Scale {
        packed_extent,
        distance_limit,
        outer_distance,
        threshold,
        scale_base,
        far_scale,
        falloff,
    })
}

/// CODE 6 +2322 and +241a object branches, after shared visibility gates.
/// Unsupported types are rejected at the host boundary.
pub fn object_glyph(kind: i8, view: i16, color: u8) -> Option<i16> {
    if view >= 2 || color == 13 {
        return None;
    }
    match kind {
        19 => Some(30),
        36 => Some(31),
        37 => Some(14),
        23 => Some(11),
        24 => Some(23),
        16 | 17 | 18 | 39 | 40 => Some(39),
        _ => None,
    }
}

/// CODE 6 +24be..+253e, restricted to single-player session mode zero.
pub fn single_player_flag_glyph(view: i16, color: u8, flash: u8) -> Option<i16> {
    (view < 2 && color != 13 && flash != 0).then_some(40)
}

/// CODE 6 +5082..+50a4. Missed intervals cause one toggle, not catch-up.
pub fn advance_flash(now: i32, deadline: i32, flash: u8) -> (i32, u8) {
    if now >= deadline {
        (now.wrapping_add(25), flash ^ 1)
    } else {
        (deadline, flash)
    }
}

/// CODE 6 +1812..+18da: optional disturbance, then ordered glyph coordinates.
/// Random words are supplied only when the stable byte is zero.
pub fn glyph_points(
    mut point: [i16; 2],
    offsets: &[[i8; 2]],
    mut stable: u8,
    now: i32,
    deadline: i32,
    random: [i16; 2],
) -> (Vec<[i16; 2]>, u8) {
    if stable == 0 {
        for axis in 0..2 {
            let value = random[axis].wrapping_abs() % 21;
            point[axis] = point[axis].wrapping_add(value).wrapping_sub(10);
        }
        if now >= deadline {
            stable = 1;
        }
    }
    let points = offsets
        .iter()
        .map(|offset| {
            [
                point[0].wrapping_add(i16::from(offset[0])),
                point[1].wrapping_add(i16::from(offset[1])),
            ]
        })
        .collect();
    (points, stable)
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub struct Disturbance {
    pub deadline: i32,
    pub stable: u8,
    pub sounds: [(i16, u32); 2],
}

/// CODE 31 +c4c..+caa, after accepted damage and health subtraction.
/// The selected difficulty comes from the original CODE 1 +890 getter.
pub fn damage_disturbance(
    victim: i8,
    local: i8,
    health: i16,
    difficulty: i16,
    now: i32,
) -> Option<Disturbance> {
    if victim != local || health > 2 {
        return None;
    }
    Some(Disturbance {
        deadline: now.wrapping_add(i32::from(difficulty.wrapping_mul(60))),
        stable: 0,
        sounds: [(1, 0x1fc1c), (1, 0x20009)],
    })
}

/// Display-only adapter. Its deterministic random stream is provisional, not
/// the original Mac OS Random implementation or shared gameplay stream.
#[derive(Debug)]
pub struct DisplayState {
    pub stable: u8,
    pub deadline: i32,
    random: u32,
}

impl Default for DisplayState {
    fn default() -> Self {
        Self {
            stable: 1,
            deadline: 0,
            random: 0x52414452,
        }
    }
}

impl DisplayState {
    pub fn disturb(&mut self, event: &Disturbance) {
        self.stable = event.stable;
        self.deadline = event.deadline;
    }

    pub fn restore(&mut self) {
        self.stable = 1;
    }

    pub fn emit(&mut self, point: [i16; 2], offsets: &[[i8; 2]], now: i32) -> Vec<[i16; 2]> {
        let mut random = [0; 2];
        if self.stable == 0 {
            for sample in &mut random {
                self.random ^= self.random << 13;
                self.random ^= self.random >> 17;
                self.random ^= self.random << 5;
                *sample = self.random as i16;
            }
        }
        let (points, stable) =
            glyph_points(point, offsets, self.stable, now, self.deadline, random);
        self.stable = stable;
        points
    }
}

/// CODE 6 +23ac..23fc: subtype color is shared by solids and radar.
/// Invalid host subtypes are rejected before the original unchecked array read.
pub fn pickup_color(subtype: i16, colors: &[u8; 4]) -> Option<u8> {
    colors.get(usize::try_from(subtype).ok()?).copied()
}

pub fn pickup_glyph(subtype: i16, view: i16, colors: &[u8; 4]) -> Option<(u16, u8)> {
    let color = pickup_color(subtype, colors)?;
    (view < 2 && color != 13).then_some((38, color))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disturbance_expires_after_first_emission_and_redraw_does_not_restart_it() {
        let mut state = DisplayState::default();
        let event = damage_disturbance(1, 1, 2, 3, 100).unwrap();
        state.disturb(&event);
        state.emit([0; 2], &[[0; 2]], 279);
        assert_eq!(state.stable, 0);
        state.emit([0; 2], &[[0; 2]], 280);
        assert_eq!(state.stable, 1);
        let seed = state.random;
        assert_eq!(state.emit([10, 20], &[[1, -1]], 280), vec![[11, 19]]);
        assert_eq!(
            state.random, seed,
            "stable rendering consumes no random words"
        );
        state.disturb(&damage_disturbance(1, 1, 1, 3, 281).unwrap());
        assert_eq!(state.deadline, 461);
        state.restore();
        assert_eq!(state.emit([10, 20], &[[0; 2]], 282), vec![[10, 20]]);
    }
}
