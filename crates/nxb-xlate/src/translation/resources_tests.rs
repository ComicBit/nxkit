use super::*;
use crate::ir::Inst;
use crate::job::{Job, parse_jobs};

fn job() -> Job {
    parse_jobs(r#"[{"key":"k","stage":"fragment","code_path":"x"}]"#)
        .unwrap()
        .remove(0)
}

fn tex(
    rm: &mut ResourceManager,
    acc: &mut Accessor,
    inst: Inst,
    ty: SamplerType,
    handle: i32,
    len: i32,
) -> SetBindingPair {
    rm.get_texture_or_image_binding(
        acc,
        inst,
        ty,
        25,
        TextureFlags::NONE,
        -1,
        handle,
        len,
        false,
    )
    .unwrap()
}

#[test]
fn binding_allocation_order() {
    let j = job();
    let mut acc = Accessor::new(&j);
    let mut rm = ResourceManager::new(ShaderStage::Fragment, None);
    // the support buffer is predefined at binding 0, the first accessor binding is 1
    assert_eq!(rm.get_constant_buffer_binding(&mut acc, 3).unwrap(), 1);
    assert_eq!(rm.get_constant_buffer_binding(&mut acc, 5).unwrap(), 2);
    assert_eq!(rm.get_constant_buffer_binding(&mut acc, 3).unwrap(), 1);
    assert_eq!(
        rm.try_get_storage_buffer_binding(&mut acc, 4, 0x30, true),
        Some(3)
    );
    assert_eq!(
        rm.try_get_storage_buffer_binding(&mut acc, 4, 0x38, false),
        Some(4)
    );
    assert_eq!(
        rm.try_get_storage_buffer_binding(&mut acc, 4, 0x30, false),
        Some(3)
    );
    let t = tex(
        &mut rm,
        &mut acc,
        Inst::TextureSample,
        SamplerType::TEXTURE_2D,
        8,
        1,
    );
    assert_eq!(t.binding, 5);
    rm.set_used_constant_buffer_binding(2);
    let cb = rm.get_constant_buffer_descriptors();
    assert_eq!(cb.len(), 1);
    assert_eq!((cb[0].binding, cb[0].slot), (2, 5));
    let sb = rm.get_storage_buffer_descriptors();
    assert_eq!(sb.len(), 2);
    assert_eq!(
        (
            sb[0].slot,
            sb[0].sb_cb_slot,
            sb[0].sb_cb_offset,
            sb[0].flags
        ),
        (0, 4, 0x30, BufferUsageFlags::WRITE)
    );
    assert_eq!(sb[1].flags, BufferUsageFlags::NONE);
    assert_eq!(rm.properties.constant_buffers.len(), 3);
    assert_eq!(rm.properties.storage_buffers.get(&3).unwrap().name, "fp_s0");
    assert_eq!(rm.try_get_constant_buffer_slot(2), Some(5));
}

#[test]
fn texture_merge_and_arrays() {
    let j = job();
    let mut acc = Accessor::new(&j);
    let mut rm = ResourceManager::new(ShaderStage::Fragment, None);
    // an array first, then plain textures: descriptors list plain ones first
    let a = tex(
        &mut rm,
        &mut acc,
        Inst::TextureSample,
        SamplerType::TEXTURE_2D,
        0x10,
        4,
    );
    let b = tex(
        &mut rm,
        &mut acc,
        Inst::TextureSample,
        SamplerType::TEXTURE_2D,
        8,
        1,
    );
    assert_eq!((a.binding, b.binding), (1, 5));
    // a query gives an inaccurate type, the accurate one wins on merge
    let q = tex(
        &mut rm,
        &mut acc,
        Inst::TextureQuerySize,
        SamplerType::TEXTURE_3D,
        8,
        1,
    );
    assert_eq!(q.binding, 5);
    let d = rm.get_texture_descriptors(true);
    assert_eq!(d.len(), 2);
    assert_eq!((d[0].binding, d[0].ty), (5, SamplerType::TEXTURE_2D));
    assert_eq!(d[1].array_length, 4);
    assert_eq!(rm.get_texture_descriptors(false).len(), 1);
    assert!(d[0].flags.0 & TextureUsageFlags::NEEDS_SCALE_VALUE.0 != 0);
    assert!(rm.is_array_of_textures_or_images(1, false));
    assert!(!rm.is_array_of_textures_or_images(5, false));
    assert_eq!(rm.find_texture_descriptor_index(1), 1);
    assert_eq!(rm.find_texture_descriptor_index(99), -1);
    assert_eq!(
        rm.try_get_cbuf_slot_and_handle_for_texture(5),
        Some((-1, 8))
    );
    let names: Vec<_> = rm
        .properties
        .textures
        .values()
        .map(|t| t.name.clone())
        .collect();
    assert_eq!(names, vec!["fp_t2d_tcb_10", "fp_t_tcb_8"]);
    assert_eq!(
        rm.get_texture_types(2, 8, false),
        vec![SamplerType::TEXTURE_2D]
    );
    assert!(rm.get_texture_types(2, 9, false).is_empty());
}

#[test]
fn image_and_local_memory() {
    let j = job();
    let mut acc = Accessor::new(&j);
    let mut rm = ResourceManager::new(ShaderStage::Compute, None);
    let i = rm
        .get_texture_or_image_binding(
            &mut acc,
            Inst::ImageStore,
            SamplerType::TEXTURE_2D,
            25,
            TextureFlags::NONE,
            2,
            0x48,
            1,
            false,
        )
        .unwrap();
    assert_eq!(i.binding, 1);
    let d = rm.get_image_descriptors(true);
    assert_eq!(
        d[0].flags.0,
        (TextureUsageFlags::NEEDS_SCALE_VALUE
            | TextureUsageFlags::RES_SCALE_UNSUPPORTED
            | TextureUsageFlags::IMAGE_STORE)
            .0
    );
    assert_eq!(
        rm.properties.images.values().next().unwrap().name,
        "cp_i_cb2_48_rgba8"
    );
    rm.set_current_local_memory(0, true);
    rm.set_current_shared_memory(10, true);
    assert_eq!(rm.properties.local_memories[0].array_length, 32);
    assert_eq!(rm.properties.shared_memories[0].array_length, 3);
    assert_eq!((rm.local_memory_id, rm.shared_memory_id), (0, 0));
    rm.set_current_local_memory(0, false);
    assert_eq!(rm.local_memory_id, -1);
}

#[test]
fn sb_slot_exhaustion() {
    let j = job();
    let mut acc = Accessor::new(&j);
    let mut rm = ResourceManager::new(ShaderStage::Vertex, None);
    for i in 0..32 {
        assert!(
            rm.try_get_storage_buffer_binding(&mut acc, 3, i * 4, false)
                .is_some()
        );
    }
    assert!(
        rm.try_get_storage_buffer_binding(&mut acc, 3, 200, false)
            .is_none()
    );
    assert_eq!(unpack_sb_cb_info(pack_sb_cb_info(3, 0x1234)), (3, 0x1234));
}
