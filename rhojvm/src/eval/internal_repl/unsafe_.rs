use rhojvm_base::id::ClassId;

use crate::{
    class_instance::{ClassInstance, FieldIndex, Instance, ReferenceInstance},
    gc::GcRef,
    jni::{JByte, JChar, JDouble, JFieldId, JFloat, JInt, JLong, JObject, JShort},
    memblock::MemoryBlockPtr,
    rv::RuntimeValuePrimitive,
    util::Env,
};

pub(crate) type JAddress = JLong;

pub(crate) extern "C" fn unsafe_allocate_memory(
    env: *mut Env<'_>,
    _this: JObject,
    size: JLong,
) -> JAddress {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let size: usize = size.try_into().expect("Out of memory error");

    let ptr = env.state.mem_blocks.allocate_block(size).unwrap();
    let ptr = ptr.get();
    let ptr = ptr as usize;
    let ptr: JAddress = ptr
        .try_into()
        .expect("Address was too large to fit into a long");

    ptr
}

unsafe fn conv_address(address: JAddress) -> MemoryBlockPtr {
    let address: usize = address.try_into().expect("Address was too high to fit into a usize. This is probably indicative of a bug in the Java code or internally.");
    let address = address as *mut u8;
    // Safety: We basically have to trust it to be valid.
    MemoryBlockPtr::new_unchecked(address)
}

pub(crate) extern "C" fn unsafe_free_memory(env: *mut Env<'_>, _this: JObject, address: JAddress) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We are only deallocating, so it just checks if the pointer exists in our allocations.
    // So not really unsafe.
    let address = unsafe { conv_address(address) };

    env.state.mem_blocks.deallocate_block(address);
}

pub(crate) extern "C" fn unsafe_get_byte(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
) -> JByte {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_i8_ne(address) }
}

pub(crate) extern "C" fn unsafe_get_short(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
) -> JShort {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_i16_ne(address) }
}

pub(crate) extern "C" fn unsafe_get_char(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
) -> JChar {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_u16_ne(address) }
}

pub(crate) extern "C" fn unsafe_get_int(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
) -> JInt {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_i32_ne(address) }
}

pub(crate) extern "C" fn unsafe_get_long(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
) -> JLong {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_i64_ne(address) }
}

pub(crate) extern "C" fn unsafe_get_float(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
) -> JFloat {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_f32_ne(address) }
}

pub(crate) extern "C" fn unsafe_get_double(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
) -> JDouble {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.get_f64_ne(address) }
}

pub(crate) extern "C" fn unsafe_put_byte(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JByte,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_i8_ne(address, value) };
}

pub(crate) extern "C" fn unsafe_put_short(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JShort,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_i16_ne(address, value) };
}

pub(crate) extern "C" fn unsafe_put_char(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JChar,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_u16_ne(address, value) };
}

pub(crate) extern "C" fn unsafe_put_int(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JInt,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_i32_ne(address, value) };
}

pub(crate) extern "C" fn unsafe_put_long(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JLong,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_i64_ne(address, value) };
}

pub(crate) extern "C" fn unsafe_put_float(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JFloat,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_f32_ne(address, value) };
}

pub(crate) extern "C" fn unsafe_put_double(
    env: *mut Env<'_>,
    _this: JObject,
    address: JAddress,
    value: JDouble,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    unsafe { env.state.mem_blocks.set_f64_ne(address, value) };
}

/// sun/misc/Unsafe
/// `public long objectFieldOffset(Field field);`
/// This just returns a unique id
pub(crate) extern "C" fn unsafe_object_field_offset(
    env: *mut Env<'_>,
    _this: JObject,
    field_ref: JObject,
) -> JLong {
    assert!(!env.is_null(), "Env was null when passed to sun/misc/Unsafe objectFieldOffset, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    let field_ref = unsafe { env.get_jobject_as_gcref(field_ref) };
    // TODO: null pointer exception
    let field_ref = field_ref.expect("objectFieldOffset's field ref was null");

    let field_class_id = env.class_names.gcid_from_bytes(b"java/lang/reflect/Field");
    let field_internal_class_id = env.class_names.gcid_from_bytes(b"rho/InternalField");
    let internal_field_id = env
        .state
        .get_field_internal_field_id(&env.class_files, field_class_id)
        .unwrap();
    let (class_id_field, field_index_field, _) = env
        .state
        .get_internal_field_ids(&env.class_files, field_internal_class_id)
        .unwrap();

    let field = env.state.gc.deref(field_ref).unwrap();
    let field = if let Instance::Reference(ReferenceInstance::Class(field)) = field {
        field
    } else {
        panic!("Bad field reference");
    };
    let internal_field_ref = field
        .fields
        .get(internal_field_id)
        .unwrap()
        .value()
        .into_reference()
        .expect("internal field should be a reference")
        .expect("Got null ptr for internal field");

    let internal_field = env.state.gc.deref(internal_field_ref).unwrap();
    let internal_field = if let ReferenceInstance::Class(field) = internal_field {
        field
    } else {
        panic!("Bad internal field reference");
    };

    let class_id_val = internal_field
        .fields
        .get(class_id_field)
        .expect("class id field should exist")
        .value()
        .into_i32()
        .expect("classid field should be i32");
    let class_id_val = ClassId::new_unchecked(class_id_val as u32);

    let field_index_val = internal_field
        .fields
        .get(field_index_field)
        .expect("field index field should exist")
        .value()
        .into_i16()
        .expect("field index field should be i16");
    let field_index_val = FieldIndex::new_unchecked(field_index_val as u16);

    // Safety: Only the JVM should fill out the Field class and so the values should be valid
    let field_id = unsafe { JFieldId::new_unchecked(class_id_val, field_index_val) };
    field_id.as_i64()
}

/// sun/misc/Unsafe
/// `int getAndAddInt(Object src, long offset, int delta);`
pub(crate) extern "C" fn unsafe_get_and_add_int(
    env: *mut Env<'_>,
    _this: JObject,
    target: JObject,
    offset: JLong,
    add_val: JInt,
) -> JInt {
    assert!(!env.is_null(), "Env was null when passed to sun/misc/Unsafe objectFieldOffset, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    let target = unsafe { env.get_jobject_as_gcref(target) };
    // We don't have to validate, since method calling should have done that
    let target: GcRef<ClassInstance> = target.expect("Null pointer exception").unchecked_as();

    let field_id = JFieldId::unchecked_from_i64(offset);
    let field_id = unsafe { field_id.into_field_id() };
    let field_id = field_id.expect("Field id was null");

    // FIXME: This is meant to be atomic!

    let target = env.state.gc.deref_mut(target).unwrap();
    // TODO: exception
    let target_val = target
        .fields
        .get_mut(field_id)
        .expect("Field offset doesn't exist for this field")
        .value_mut();
    let current_val = (*target_val).into_i32().expect("Field value should be int");

    *target_val = RuntimeValuePrimitive::I32(current_val.overflowing_add(add_val).0).into();

    current_val
}
