//! The vertex pad (`vs_pad`): the outputs the paired pixel shaders read and the vertex shader does not write.
//!
//! Metal links a pixel shader only when every user input is a vertex output. Maxwell reads a default of
//! `(0, 0, 0, 1)` for an attribute the previous stage did not write. The pad gives the vertex shader such outputs with
//! the type the pixel shader declares and the default value. The interpolation qualifier is an attribute of the
//! pixel shader input only, a vertex output cannot carry it.

use super::super::StageInputs;
use super::super::io::Plan;
use std::collections::BTreeMap;

/// One padded output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pad {
    /// The varying location.
    pub location: u32,
    /// The MSL type of the member.
    pub ty: String,
}

impl Pad {
    /// The member declaration in `main0_out`.
    pub fn member(&self) -> String {
        format!(
            "{} dm_pad{l} [[user(locn{l})]];",
            self.ty,
            l = self.location
        )
    }

    /// The statement that starts the output with its default.
    pub fn init(&self) -> String {
        let v = if self.ty.starts_with("float") && self.ty.ends_with('4') {
            "float4(0.0, 0.0, 0.0, 1.0)".to_string()
        } else {
            format!("{}(0)", self.ty)
        };
        format!("out.dm_pad{} = {v};", self.location)
    }
}

/// Add the pad members to the plan and return the padded locations in order. The first partner that reads a
/// location decides its type.
pub fn apply(plan: &mut Plan, partners: &[StageInputs]) -> Vec<u32> {
    let mut want: BTreeMap<u32, &str> = BTreeMap::new();
    for v in partners.iter().flat_map(|p| p.varyings.iter()) {
        want.entry(v.location).or_insert(v.msl_type.as_str());
    }
    let mut miss = Vec::new();
    for (l, ty) in want {
        if plan.user_locs.contains(&l) {
            continue;
        }
        let pad = Pad {
            location: l,
            ty: ty.to_string(),
        };
        plan.out_members.push(pad.member());
        plan.pads.push(pad);
        miss.push(l);
    }
    miss
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msl::Varying;

    fn partner(v: &[(u32, &str, &str)]) -> StageInputs {
        StageInputs {
            varyings: v
                .iter()
                .map(|(l, t, i)| Varying {
                    location: *l,
                    msl_type: (*t).into(),
                    interpolation: (*i).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn missing_locations_are_padded_once_with_the_partner_type() {
        let mut p = Plan {
            user_locs: vec![1],
            out_members: vec!["float4 gl_Position [[position]];".into()],
            ..Plan::default()
        };
        let a = partner(&[(1, "float4", ""), (3, "float4", "flat")]);
        let b = partner(&[(3, "float4", ""), (2, "int4", "flat")]);
        assert_eq!(apply(&mut p, &[a, b]), vec![2, 3]);
        assert_eq!(p.out_members[1], "int4 dm_pad2 [[user(locn2)]];");
        assert_eq!(p.out_members[2], "float4 dm_pad3 [[user(locn3)]];");
        assert_eq!(p.pads[0].init(), "out.dm_pad2 = int4(0);");
        assert_eq!(
            p.pads[1].init(),
            "out.dm_pad3 = float4(0.0, 0.0, 0.0, 1.0);"
        );
    }

    #[test]
    fn nothing_is_padded_without_partners() {
        let mut p = Plan::default();
        assert!(apply(&mut p, &[]).is_empty());
        assert!(p.pads.is_empty());
    }
}
