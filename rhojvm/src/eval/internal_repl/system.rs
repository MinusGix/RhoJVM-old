use usize_cast::IntoUsize;

use crate::{
    class_instance::{Instance, PrimitiveArrayInstance, ReferenceInstance},
    gc::GcRef,
    jni::{JInt, JObject},
    util::Env,
};

/// java/lang/System
/// `public static void arraycopy(Object src, int src_pos, Object dest, int dest_position, int
/// length)`
pub(crate) extern "C" fn system_arraycopy(
    env: *mut Env<'_>,
    _this: JObject,
    source: JObject,
    source_start: JInt,
    destination: JObject,
    destination_start: JInt,
    count: JInt,
) {
    assert!(
        !env.is_null(),
        "System arraycopy got a null env, this is indicative of an internal bug."
    );

    let env = unsafe { &mut *env };

    let source_ref = unsafe { env.get_jobject_as_gcref(source) };
    let source_ref = source_ref.expect("null pointer");

    let destination_ref = unsafe { env.get_jobject_as_gcref(destination) };
    let destination_ref = destination_ref.expect("null pointer");

    let source_inst = env.state.gc.deref(source_ref).unwrap();
    let destination_inst = env.state.gc.deref(destination_ref).unwrap();
    match (source_inst, destination_inst) {
        (_, Instance::StaticClass(_)) | (Instance::StaticClass(_), _) => {
            panic!("Should not be a static class")
        }
        (Instance::Reference(src), Instance::Reference(dest)) => match (dest, src) {
            (ReferenceInstance::PrimitiveArray(_), ReferenceInstance::PrimitiveArray(_)) => {
                system_arraycopy_primitive(
                    env,
                    source_ref.unchecked_as::<PrimitiveArrayInstance>(),
                    source_start,
                    destination_ref.unchecked_as::<PrimitiveArrayInstance>(),
                    destination_start,
                    count,
                );
            }
            (ReferenceInstance::ReferenceArray(_), ReferenceInstance::ReferenceArray(_)) => todo!(),
            (ReferenceInstance::PrimitiveArray(_), _)
            | (_, ReferenceInstance::PrimitiveArray(_)) => todo!("Wrong types"),
            (ReferenceInstance::ReferenceArray(_), _)
            | (_, ReferenceInstance::ReferenceArray(_)) => todo!("Wrong types"),
            _ => panic!("Throw exception, this should be an array"),
        },
    };
}

fn system_arraycopy_primitive(
    env: &mut Env,
    source_ref: GcRef<PrimitiveArrayInstance>,
    source_start: i32,
    destination_ref: GcRef<PrimitiveArrayInstance>,
    destination_start: i32,
    count: i32,
) {
    if source_start < 0 || destination_start < 0 {
        todo!("One of the starts was negative");
    } else if count < 0 {
        todo!("Count was an negative");
    }

    let source_start = source_start.unsigned_abs().into_usize();
    let destination_start = destination_start.unsigned_abs().into_usize();
    let count = count.unsigned_abs().into_usize();

    // TODO: We should only need to clone if source == destination!
    let source = env.state.gc.deref(source_ref).unwrap().clone();

    let destination = env.state.gc.deref_mut(destination_ref).unwrap();

    if source.element_type != destination.element_type {
        todo!("Error about incompatible types")
    }

    // TODO: overflow checks
    let source_end = source_start + count;
    let destination_end = destination_start + count;

    let source_slice = if let Some(source_slice) = source.elements.get(source_start..source_end) {
        source_slice
    } else {
        todo!("Exception about source start exceeding length");
    };

    let destination_slice = if let Some(destination_slice) = destination
        .elements
        .get_mut(destination_start..destination_end)
    {
        destination_slice
    } else {
        todo!("Exception about destination start exceeding length");
    };

    assert_eq!(source_slice.len(), destination_slice.len());

    for (dest, src) in destination_slice.iter_mut().zip(source_slice.iter()) {
        *dest = *src;
    }
}
