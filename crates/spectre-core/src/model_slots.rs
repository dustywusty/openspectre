//! Collision geometry assembled through recovered loader dispatch.
//! Caller load order is explicit. Original startup ordering is not yet recovered.
use crate::{
    model::PackedShape,
    recovered::{self, ModelLoadRequest, ModelSlotState},
    scene::Catalog,
};

#[derive(Clone, Default)]
pub struct ModelSlots {
    pub state: ModelSlotState,
    /// One-based slot keys. Missing geometry is never replaced with preview bounds.
    pub geometry: std::collections::BTreeMap<i16, PackedShape>,
}
impl ModelSlots {
    /// Unsupported reconstruction paths leave this library unchanged. This is
    /// a host guard, not original resource-failure behavior (which is retained).
    pub fn load_type(&mut self, catalog: &Catalog, kind: usize) -> Option<()> {
        let mut pending = self.clone();
        pending.load_type_in_place(catalog, kind)?;
        *self = pending;
        Some(())
    }
    fn load_type_in_place(&mut self, catalog: &Catalog, kind: usize) -> Option<()> {
        let already_loaded = self.state.current.get(kind).copied()? != -1;
        let geometry = &mut self.geometry;
        self.state.load_type(kind, &mut |state, request| {
            match request {
                ModelLoadRequest::Selected(kind) => {
                    // Current campaign preview uses the third selection column.
                    // Reservation occurs inside load_selected before inner dispatch.
                    state.load_selected(
                        kind,
                        Some(&catalog.choices),
                        false,
                        false,
                        &mut |entry, id, slot, kind| {
                            Self::inner(catalog, geometry, entry, id, slot, kind)
                        },
                    )?;
                    Some(0)
                }
                ModelLoadRequest::Resource { kind, id, slot } => {
                    Self::inner(catalog, geometry, 0x165e, id, slot, kind)
                }
                ModelLoadRequest::Halve { slot } => {
                    let shape = geometry.get_mut(&(slot as i16))?;
                    shape.dimensions = recovered::override_collision_dimensions(
                        shape.dimensions,
                        recovered::CollisionDimensionOverride::Half,
                    );
                    shape.candidate_radius /= 2;
                    Some(0)
                }
            }
        })?;
        if kind == 12 && !already_loaded {
            let source = self.state.current[2];
            let destination = self.state.current[12];
            if source != -1 && destination != -1 {
                let mut dimensions = vec![[0; 2]; 100];
                let mut radii = vec![0; 100];
                let source_shape = self.geometry.get(&(source as i16))?;
                dimensions[source as usize - 1] = source_shape.dimensions;
                radii[source as usize - 1] = source_shape.candidate_radius;
                recovered::copy_flag_collision(destination, source, &mut dimensions, &mut radii)?;
                let flag = self.geometry.get_mut(&(destination as i16))?;
                flag.dimensions = dimensions[destination as usize - 1];
                flag.candidate_radius = radii[destination as usize - 1];
            }
        }
        Some(())
    }
    fn inner(
        catalog: &Catalog,
        geometry: &mut std::collections::BTreeMap<i16, PackedShape>,
        entry: u16,
        id: i16,
        slot: i16,
        kind: usize,
    ) -> Option<i16> {
        match entry {
            0x165e => {
                let Some(shape) = catalog.packed_shapes.get(&id) else {
                    return Some(0);
                };
                if !(1..=100).contains(&slot) {
                    return None;
                }
                geometry.insert(slot, shape.clone());
                Some(1)
            }
            0x15ba => recovered::load_animation_models(
                catalog.animations.get(&id).map(Vec::as_slice),
                slot,
                kind as u8,
                0,
                &mut |entry, id, slot, kind| {
                    Self::inner(catalog, geometry, entry, id, slot, kind as usize)
                },
            ),
            0x133a => {
                // These builders use the slot for their fixed object type. Only
                // the matching selection-table path is implemented here.
                let mut shape = match (kind, id) {
                    (0, 1) | (4, 5) => PackedShape::fixed_obstacle(id, false)?.0,
                    (1, 2) => PackedShape::tetrahedron(),
                    (5, 6) => PackedShape::ammo_dump(),
                    (12, 12) => PackedShape::flag(false).0,
                    (2, 3) => PackedShape::octagonal_column(&recovered::candidate_unit_table()),
                    _ => return None,
                };
                if !(1..=100).contains(&slot) {
                    return None;
                }
                if id == 5 {
                    shape.dimensions = recovered::override_collision_dimensions(
                        shape.dimensions,
                        recovered::CollisionDimensionOverride::TenUnits,
                    );
                    // CODE 19 +0x223a..+0x2268: ceil(0x00038666).
                    shape.candidate_radius = 4;
                }
                geometry.insert(slot, shape);
                Some(1)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flag_bounds_copy_only_an_existing_type_two_slot() {
        let mut catalog = Catalog {
            choices: vec![[0; 3]; 47],
            ..Catalog::default()
        };
        catalog.choices[2] = [3; 3];
        catalog.choices[12] = [12; 3];
        let mut slots = ModelSlots::default();
        slots.load_type(&catalog, 12).unwrap();
        let base = slots.geometry[&1].clone();
        slots.load_type(&catalog, 2).unwrap();
        slots.load_type(&catalog, 12).unwrap();
        assert_eq!(
            slots.geometry[&1].dimensions, base.dimensions,
            "already-loaded flag is unchanged"
        );
        let mut slots = ModelSlots::default();
        slots.load_type(&catalog, 2).unwrap();
        slots.load_type(&catalog, 12).unwrap();
        let flag = &slots.geometry[&2];
        let column = &slots.geometry[&1];
        assert_eq!(flag.dimensions, column.dimensions);
        assert_eq!(flag.candidate_radius, column.candidate_radius);
        assert_eq!(
            flag.vertices, base.vertices,
            "collision copy preserves visible geometry"
        );
        assert_ne!(flag.dimensions, base.dimensions);
    }
}
