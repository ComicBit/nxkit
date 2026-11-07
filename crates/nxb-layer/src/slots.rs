//! Descriptor reflection and the Metal slot of each resource, as `vk_shaders.py` numbers them.
//!
//! The resources are numbered in (set, binding) order per Metal class: buffers for uniform and storage buffers and the
//! push-constant block, textures for images and combined image samplers, samplers for samplers and combined image samplers.
//! An array of `k` descriptors takes `k` consecutive slots. A combined image sampler at texture slot `n` also takes sampler
//! slot `n`. The push-constant block gets the next buffer slot after the descriptor buffers.

use std::collections::{BTreeMap, BTreeSet};

use naga::back::msl::{BindSamplerTarget, BindTarget, EntryPointResources};
use naga::{AddressSpace, ArraySize, GlobalVariable, Handle, Module, ResourceBinding, TypeInner, valid::ModuleInfo};

/// The highest slot count per class: 16 buffers, 31 textures and 16 samplers.
pub(crate) const LIMITS: (u32, u32, u32) = (16, 31, 16);

/// One descriptor binding of the shader and its Metal slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Res {
    /// `u` uniform buffer, `b` storage buffer, `t` image, `s` sampler, `c` combined image sampler, `p` push constants.
    pub kind: char,
    pub set: u32,
    pub binding: u32,
    /// The number of descriptors, more than 1 for an array.
    pub count: u32,
    pub slot: u32,
    /// The globals of this binding: one, or the image and the sampler of a combined image sampler.
    pub globals: Vec<Handle<GlobalVariable>>,
}

impl Res {
    /// The table token `kind:set.binding=slot[*count]`.
    pub(crate) fn token(&self) -> String {
        let n = if self.count > 1 { format!("*{}", self.count) } else { String::new() };
        format!("{}:{}.{}={}{n}", self.kind, self.set, self.binding, self.slot)
    }
}

/// The resources and the slot assignment of a module.
#[derive(Clone, Debug, Default)]
pub(crate) struct Slots {
    pub res: Vec<Res>,
    pub push: Option<Res>,
}

fn kind_and_count(m: &Module, g: &GlobalVariable, combined: &BTreeSet<(u32, u32)>, rb: &ResourceBinding) -> Option<(char, u32)> {
    let (inner, count) = match m.types[g.ty].inner {
        TypeInner::BindingArray { base, size } => {
            let n = match size {
                ArraySize::Constant(n) => n.get(),
                _ => 1,
            };
            (&m.types[base].inner, n)
        }
        ref other => (other, 1),
    };
    let kind = match (g.space, inner) {
        (AddressSpace::Uniform, _) => 'u',
        (AddressSpace::Storage { .. }, _) => 'b',
        (AddressSpace::Handle, TypeInner::Image { .. }) if combined.contains(&(rb.group, rb.binding)) => 'c',
        (AddressSpace::Handle, TypeInner::Sampler { .. }) if combined.contains(&(rb.group, rb.binding)) => 'c',
        (AddressSpace::Handle, TypeInner::Image { .. }) => 't',
        (AddressSpace::Handle, TypeInner::Sampler { .. }) => 's',
        _ => return None,
    };
    Some((kind, count))
}

/// Numbers the module's resources. `combined` lists the (set, binding) pairs that were combined image samplers before
/// `crate::spirv::split_combined`. An error names a class whose slots run over Metal's limits.
pub(crate) fn assign(m: &Module, combined: &BTreeSet<(u32, u32)>) -> Result<Slots, String> {
    let mut by_pair: BTreeMap<(u32, u32), Res> = BTreeMap::new();
    let mut push = None;
    for (h, g) in m.global_variables.iter() {
        if g.space == AddressSpace::Immediate {
            push = Some(Res { kind: 'p', set: 0, binding: 0, count: 1, slot: 0, globals: vec![h] });
            continue;
        }
        let Some(rb) = g.binding else { continue };
        let Some((kind, count)) = kind_and_count(m, g, combined, &rb) else { continue };
        let e = by_pair.entry((rb.group, rb.binding)).or_insert(Res {
            kind,
            set: rb.group,
            binding: rb.binding,
            count,
            slot: 0,
            globals: Vec::new(),
        });
        if e.kind != kind {
            return Err(format!("set {} binding {} holds both a {} and a {}", rb.group, rb.binding, e.kind, kind));
        }
        e.globals.push(h);
    }
    let (mut buf, mut tex, mut smp) = (0, 0, 0);
    let mut res: Vec<Res> = by_pair.into_values().collect();
    for r in &mut res {
        let next = match r.kind {
            'u' | 'b' => &mut buf,
            't' | 'c' => &mut tex,
            _ => &mut smp,
        };
        r.slot = *next;
        *next += r.count;
        if r.kind == 'c' {
            smp = smp.max(r.slot + r.count);
        }
    }
    if let Some(p) = push.as_mut() {
        p.slot = buf;
        buf += 1;
    }
    if buf > LIMITS.0 || tex > LIMITS.1 || smp > LIMITS.2 {
        return Err(format!("resource slots exceeded: buffer {buf}, texture {tex}, sampler {smp}"));
    }
    Ok(Slots { res, push })
}

/// OpenGL's numbering: each resource's Metal slot is its binding, as `--msl-decoration-binding` gives it. `combined` as for
/// [`assign`]. A buffer or sampler slot above 15 is an error.
pub(crate) fn by_binding(m: &Module, combined: &BTreeSet<(u32, u32)>) -> Result<Slots, String> {
    let mut s = assign(m, combined).map_err(|e| e.replace("resource slots exceeded", "resources"))?;
    for r in &mut s.res {
        r.slot = r.binding;
        if r.slot + r.count > 16 && r.kind != 't' {
            return Err("a buffer or sampler slot above 15".into());
        }
    }
    Ok(s)
}

impl Slots {
    /// naga's binding map for one entry point.
    pub(crate) fn binding_map(&self) -> EntryPointResources {
        let mut ep = EntryPointResources::default();
        for r in &self.res {
            let mut t = BindTarget::default();
            match r.kind {
                'u' | 'b' => t.buffer = Some(r.slot as u8),
                't' => t.texture = Some(r.slot as u8),
                's' => t.sampler = Some(BindSamplerTarget::Resource(r.slot as u8)),
                _ => {
                    t.texture = Some(r.slot as u8);
                    t.sampler = Some(BindSamplerTarget::Resource(r.slot as u8));
                }
            }
            t.mutable = r.kind == 'b';
            ep.resources.insert(ResourceBinding { group: r.set, binding: r.binding }, t);
        }
        ep.immediates_buffer = self.push.as_ref().map(|p| p.slot as u8);
        ep
    }

    /// The table tokens of the resources that entry point `ep` uses, the push constants last.
    pub(crate) fn tokens(&self, info: &ModuleInfo, ep: usize) -> Vec<String> {
        let fi = info.get_entry_point(ep);
        let used = |r: &Res| r.globals.iter().any(|&g| !fi[g].is_empty());
        self.res.iter().chain(self.push.iter()).filter(|r| used(r)).map(Res::token).collect()
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
