use std::rc::Rc;

use classfile_parser::{class_parser_opt, parser::ParseData};
use indexmap::IndexMap;
use rhojvm_base::{
    class::{AnonBasedClassFileData, ClassFileInfo},
    constant_pool::{ConstantInfoPool, MapConstantPool, ShadowConstantPool},
    data::class_file_loader::LoadClassFileError,
    id::ClassId,
};

use crate::{
    class_instance::{
        ClassInstance, FieldIndex, Instance, PrimitiveArrayInstance, ReferenceArrayInstance,
        ReferenceInstance, StaticFormInstance,
    },
    initialize_class,
    jni::{JByte, JChar, JDouble, JFieldId, JFloat, JInt, JLong, JObject, JShort},
    memblock::MemoryBlockPtr,
    rv::{RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::{make_class_form_of, ref_info, Env},
};

pub(crate) type JAddress = JLong;

pub(crate) extern "C" fn unsafe_address_size(env: *mut Env<'_>, _this: JObject) -> JInt {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let _env = unsafe { &mut *env };

    if cfg!(target_pointer_width = "64") {
        8
    } else if cfg!(target_pointer_width = "32") {
        4
    } else {
        panic!("Unsupported pointer width");
    }
}

pub(crate) extern "C" fn unsafe_page_size(env: *mut Env<'_>, _this: JObject) -> JInt {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let _env = unsafe { &mut *env };

    page_size::get().try_into().unwrap()
}

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

/// `void setMemory(long address, long count, byte value)`
pub(crate) extern "C" fn unsafe_set_memory_n(
    env: *mut Env<'_>,
    _this: JObject,
    address: JLong,
    count: JLong,
    value: JByte,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    // Safety: We basically have to trust that it is valid.
    let address = unsafe { conv_address(address) };

    if count <= 0 {
        return;
    }

    let count = count as u64;
    let count: usize = count.try_into().unwrap();

    let value = u8::from_be_bytes(i8::to_be_bytes(value));

    // TODO: atomic
    unsafe {
        env.state.mem_blocks.write_repeat(address, count, value);
    }
}

pub(crate) extern "C" fn unsafe_copy_memory(
    env: *mut Env<'_>,
    _this: JObject,
    src: JObject,
    src_offset: JLong,
    dest: JObject,
    dest_offset: JLong,
    count: JLong,
) {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let src = unsafe { env.get_jobject_as_gcref(src) };

    let dest = unsafe { env.get_jobject_as_gcref(dest) };

    if count <= 0 {
        return;
    }

    let count = count as u64;
    let count: usize = count.try_into().unwrap();

    if src_offset < 0 {
        panic!("Unsafe#copyMemory: src_offset was negative");
    }

    // TODO: Support more general src/dest
    if let Some(dest) = dest {
        let dest_offset = dest_offset as u64;
        let dest_offset: usize = dest_offset.try_into().unwrap();

        let dest = dest.unchecked_as::<PrimitiveArrayInstance>();
        let Some(dest_inst) = env.state.gc.deref(dest) else {
            panic!("Failed to find dest, expected primitive array instance. Other kinds are not supported at this time");
        };

        assert_eq!(dest_inst.element_type, RuntimeTypePrimitive::I8);

        if let Some(src) = src {
            let src_offset = src_offset as u64;
            let src_offset: usize = src_offset.try_into().unwrap();

            let src = src.unchecked_as::<PrimitiveArrayInstance>();
            let Some(src) = env.state.gc.deref(src) else {
                panic!("Failed to find src, expected primitive array instance. Other kinds are not supported at this time");
            };

            assert_eq!(src.element_type, RuntimeTypePrimitive::I8);

            let src_bytes = src
                .elements
                .iter()
                .skip(src_offset)
                .take(count)
                .copied()
                .collect::<Vec<_>>();

            let dest = env.state.gc.deref_mut(dest).unwrap();

            for (i, byte) in src_bytes.iter().enumerate() {
                dest.elements[dest_offset + i] = *byte;
            }
        } else {
            let src_address = unsafe { conv_address(src_offset) };

            let src_bytes = unsafe { env.state.mem_blocks.read_slice(src_address, count) };

            let dest = env.state.gc.deref_mut(dest).unwrap();

            for (i, byte) in src_bytes.iter().enumerate() {
                let byte = i8::from_be_bytes(u8::to_be_bytes(*byte));
                dest.elements[dest_offset + i] = RuntimeValuePrimitive::I8(byte);
            }
        }
    } else {
        // dest is null, so dest_offset is really just an address
        let address = unsafe { conv_address(dest_offset) };

        if let Some(src) = src {
            // Note: this code assumes that ARRAY_{TYPE}_BASE_OFFSET is 0
            let src_offset = src_offset as u64;
            let src_offset: usize = src_offset.try_into().unwrap();

            let src = src.unchecked_as::<PrimitiveArrayInstance>();
            let Some(src) = env.state.gc.deref(src) else {
                panic!("Failed to find src, expected primitive array instance. Other kinds are not supported at this time");
            };
            assert_eq!(src.element_type, RuntimeTypePrimitive::I8);

            let src_bytes = src
                .elements
                .iter()
                .skip(src_offset)
                .take(count)
                .map(|x| x.into_byte().unwrap())
                .map(|x| u8::from_be_bytes(i8::to_be_bytes(x)))
                .collect::<Vec<u8>>();
            unsafe { env.state.mem_blocks.write_slice(address, &src_bytes) };
        } else {
            // Src is null, so src_offset is really just an address
            let src_address = unsafe { conv_address(src_offset) };

            let src_bytes: &[u8] = unsafe { env.state.mem_blocks.read_slice(src_address, count) };

            unsafe { env.state.mem_blocks.write_slice(address, src_bytes) };
        }
    }
}

pub(crate) extern "C" fn unsafe_get_byte_ptr(
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

pub(crate) extern "C" fn unsafe_get_short_ptr(
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

pub(crate) extern "C" fn unsafe_get_char_ptr(
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

pub(crate) extern "C" fn unsafe_get_int_ptr(
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

pub(crate) extern "C" fn unsafe_get_long_ptr(
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

pub(crate) extern "C" fn unsafe_get_float_ptr(
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

pub(crate) extern "C" fn unsafe_get_double_ptr(
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

pub(crate) extern "C" fn unsafe_put_byte_ptr(
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

pub(crate) extern "C" fn unsafe_put_short_ptr(
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

pub(crate) extern "C" fn unsafe_put_char_ptr(
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

pub(crate) extern "C" fn unsafe_put_int_ptr(
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

pub(crate) extern "C" fn unsafe_put_long_ptr(
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

pub(crate) extern "C" fn unsafe_put_float_ptr(
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

pub(crate) extern "C" fn unsafe_put_double_ptr(
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

fn modify_field_value(
    env: &mut Env,
    target: JObject,
    offset: JLong,
    modify_with: impl FnOnce(RuntimeValue<ReferenceInstance>) -> RuntimeValue<ReferenceInstance>,
) {
    fn modify_field(
        class: &mut ClassInstance,
        offset: JLong,
        modify_with: impl FnOnce(RuntimeValue<ReferenceInstance>) -> RuntimeValue<ReferenceInstance>,
    ) {
        let field_id = JFieldId::unchecked_from_i64(offset);
        let field_id = unsafe { field_id.into_field_id() };
        let field_id = field_id.expect("Field id was null");

        let field = class.fields.get_mut(field_id).unwrap();
        let field_value = field.value_mut();
        let new_value = modify_with(*field_value);
        *field_value = new_value;
    }

    let target = unsafe { env.get_jobject_as_gcref(target) };
    let target = target.expect("Null pointer exception");

    let target = env.state.gc.deref_mut(target).unwrap();
    match target {
        Instance::StaticClass(_) => todo!(),
        Instance::Reference(class) => match class {
            ReferenceInstance::Class(class) => modify_field(class, offset, modify_with),
            ReferenceInstance::StaticForm(form) => {
                modify_field(&mut form.inner, offset, modify_with);
            }
            ReferenceInstance::Thread(thread) => {
                modify_field(&mut thread.inner, offset, modify_with);
            }
            ReferenceInstance::MethodHandle(handle) => {
                modify_field(&mut handle.inner, offset, modify_with);
            }
            ReferenceInstance::MethodHandleInfo(info) => {
                modify_field(&mut info.inner, offset, modify_with);
            }
            // For arrays, the offset is currently just the index
            ReferenceInstance::PrimitiveArray(arr) => {
                let offset = usize::try_from(offset).unwrap();
                let arr_value = arr.elements.get_mut(offset).unwrap();
                let new_value = modify_with((*arr_value).into());
                let new_value = new_value.into_primitive().unwrap();
                let new_value_type = new_value.runtime_type();
                assert_eq!(arr.element_type, new_value_type);
                *arr_value = new_value;
            }
            ReferenceInstance::ReferenceArray(arr) => {
                let offset = usize::try_from(offset).unwrap();
                let arr_value = arr.elements.get_mut(offset).unwrap();
                let new_value = if let Some(arr_value) = arr_value {
                    modify_with(RuntimeValue::Reference(*arr_value))
                } else {
                    modify_with(RuntimeValue::NullReference)
                };
                let new_value = new_value.into_reference().unwrap();
                // TODO: assert/check that the type of the stored reference is valid!
                *arr_value = new_value;
            }
        },
    };
}

fn get_field_value(
    env: &mut Env,
    target: JObject,
    offset: JLong,
) -> RuntimeValue<ReferenceInstance> {
    fn get_field(class: &ClassInstance, offset: JLong) -> RuntimeValue<ReferenceInstance> {
        let field_id = JFieldId::unchecked_from_i64(offset);
        let field_id = unsafe { field_id.into_field_id() };
        let field_id = field_id.expect("Field id was null");

        class
            .fields
            .get(field_id)
            .expect("Field offset doesn't exist on the target")
            .value()
    }

    let target = unsafe { env.get_jobject_as_gcref(target) };
    let target = target.expect("Null pointer exception");

    let target = env.state.gc.deref(target).unwrap();
    match target {
        Instance::StaticClass(_) => todo!(),
        Instance::Reference(class) => match class {
            ReferenceInstance::Class(class) => get_field(class, offset),
            ReferenceInstance::StaticForm(form) => get_field(&form.inner, offset),
            ReferenceInstance::Thread(thread) => get_field(&thread.inner, offset),
            ReferenceInstance::MethodHandle(handle) => get_field(&handle.inner, offset),
            ReferenceInstance::MethodHandleInfo(info) => get_field(&info.inner, offset),
            ReferenceInstance::PrimitiveArray(arr) => {
                let offset = usize::try_from(offset).unwrap();
                let arr_value = arr.elements.get(offset).unwrap();
                (*arr_value).into()
            }
            ReferenceInstance::ReferenceArray(arr) => {
                let offset = usize::try_from(offset).unwrap();
                let arr_value = arr.elements.get(offset).unwrap();
                if let Some(re) = *arr_value {
                    RuntimeValue::Reference(re)
                } else {
                    RuntimeValue::NullReference
                }
            }
        },
    }
}

pub(crate) extern "C" fn unsafe_get_int(
    env: *mut Env<'_>,
    _this: JObject,
    target: JObject,
    offset: JLong,
) -> JInt {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    get_field_value(env, target, offset).into_i32().unwrap()
}

pub(crate) extern "C" fn unsafe_put_int(
    env: *mut Env<'_>,
    _this: JObject,
    target: JObject,
    offset: JLong,
    value: JInt,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    modify_field_value(env, target, offset, |val| {
        assert!(val.into_i32().is_some());
        RuntimeValuePrimitive::I32(value).into()
    });
}

pub(crate) extern "C" fn unsafe_get_long(
    env: *mut Env<'_>,
    _this: JObject,
    target: JObject,
    offset: JLong,
) -> JLong {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    get_field_value(env, target, offset).into_i64().unwrap()
}

pub(crate) extern "C" fn unsafe_put_long(
    env: *mut Env<'_>,
    _this: JObject,
    target: JObject,
    offset: JLong,
    value: JLong,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    modify_field_value(env, target, offset, |val| {
        assert!(val.into_i64().is_some());
        RuntimeValuePrimitive::I64(value).into()
    });
}

pub(crate) extern "C" fn unsafe_get_object(
    env: *mut Env,
    _: JObject,
    target: JObject,
    offset: JLong,
) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let re = get_field_value(env, target, offset)
        .into_reference()
        .unwrap();
    if let Some(re) = re {
        unsafe { env.get_local_jobject_for(re.into_generic()) }
    } else {
        JObject::null()
    }
}

pub(crate) extern "C" fn unsafe_put_object(
    env: *mut Env,
    _: JObject,
    target: JObject,
    offset: JLong,
    value: JObject,
) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let value = unsafe { env.get_jobject_as_gcref(value) };

    modify_field_value(env, target, offset, |val| {
        assert!(val.into_reference().is_some());
        if let Some(value) = value {
            RuntimeValue::Reference(value.unchecked_as())
        } else {
            RuntimeValue::NullReference
        }
    });
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

    // TODO: atomically
    let mut gotten_value = None;
    modify_field_value(env, target, offset, |val| {
        let current_val = val.into_i32().expect("Field value should be int");
        gotten_value = Some(current_val);
        RuntimeValuePrimitive::I32(current_val.overflowing_add(add_val).0).into()
    });

    if let Some(gotten_value) = gotten_value {
        gotten_value
    } else {
        panic!();
    }
}

/// sun/misc/Unsafe
/// `long getAndAddLong(Object src, long offset, long delta);`
pub(crate) extern "C" fn unsafe_get_and_add_long(
    env: *mut Env<'_>,
    _this: JObject,
    target: JObject,
    offset: JLong,
    add_val: JLong,
) -> JLong {
    assert!(!env.is_null(), "Env was null when passed to sun/misc/Unsafe objectFieldOffset, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    // TODO: atomically
    let mut gotten_value = None;
    modify_field_value(env, target, offset, |val| {
        let current_val = val.into_i64().expect("Field value should be long");
        gotten_value = Some(current_val);
        RuntimeValuePrimitive::I64(current_val.overflowing_add(add_val).0).into()
    });

    if let Some(gotten_value) = gotten_value {
        gotten_value
    } else {
        panic!();
    }
}

pub(crate) extern "C" fn unsafe_define_anon_class(
    env: *mut Env<'_>,
    _this: JObject,
    base_class: JObject,
    data: JObject,
    patches: JObject,
) -> JObject {
    assert!(!env.is_null(), "Env was null when passed to sun/misc/Unsafe defineAnonymousClass, which is indicative of an internal bug.");
    let env = unsafe { &mut *env };

    let base_class = unsafe { env.get_jobject_as_gcref(base_class) };
    let data = unsafe { env.get_jobject_as_gcref(data) };
    let patches = unsafe { env.get_jobject_as_gcref(patches) };

    let base_class = base_class.unwrap();
    let Some(base_class) = env.state.gc.checked_as(base_class) else {
        panic!("Base class is not a class");
    };
    let base_class: &StaticFormInstance = env.state.gc.deref(base_class).unwrap();
    let base_class_of = base_class.of;
    let base_class_of = base_class_of.into_reference().unwrap();

    let data = data.unwrap();
    let Some(data) = env.state.gc.checked_as(data) else {
        panic!("Data is not a byte array");
    };
    let data: &PrimitiveArrayInstance = env.state.gc.deref(data).unwrap();
    let data = data
        .elements
        .iter()
        .map(|x| x.into_byte().unwrap())
        .map(|x| u8::from_be_bytes(i8::to_be_bytes(x)))
        .collect::<Vec<u8>>();
    let data: Rc<[u8]> = Rc::from(data);

    let patches = if let Some(patches) = patches {
        let Some(patches) = env.state.gc.checked_as(patches) else {
            panic!("Patches is not a byte array");
        };

        // TODO: Support constant pool patches
        let patches: &ReferenceArrayInstance = env.state.gc.deref(patches).unwrap();
        assert!(
            patches.elements.is_empty(),
            "Patches is not empty. Not implemented yet."
        );

        MapConstantPool::default()
    } else {
        MapConstantPool::default()
    };

    let opt = {
        let (rem_data, class_file) = class_parser_opt(ParseData::new(&data))
            .map_err(|x| format!("{:?}", x))
            .map_err(LoadClassFileError::ClassFileParseError)
            .unwrap();
        debug_assert!(rem_data.is_empty());

        class_file
    };

    let const_pool = ShadowConstantPool::new(patches, opt.const_pool.clone());

    let new_class_name = const_pool.get_t(opt.this_class).unwrap();
    let new_class_name = const_pool
        .get_t(new_class_name.name_index)
        .unwrap()
        .as_bytes(&data);
    let new_class_id = env.class_names.gcid_from_bytes(new_class_name);

    // let new_class_id = env.class_names.init_new_id(true);
    let shadow_text = IndexMap::default();
    let class_file = AnonBasedClassFileData::new(
        new_class_id,
        base_class_of,
        data.clone(),
        opt,
        const_pool,
        shadow_text,
    );

    env.class_files
        .set_at_unchecked(new_class_id, ClassFileInfo::AnonBased(class_file));

    let unsafe_id = env.class_names.gcid_from_bytes(b"sun/misc/Unsafe");
    let static_form = make_class_form_of(env, unsafe_id, new_class_id).unwrap();
    let Some(static_form) = env.state.extract_value(static_form) else {
        return JObject::null();
    };

    let res = initialize_class(env, new_class_id).unwrap().into_value();
    let _res = env.state.extract_value(res).unwrap();

    tracing::info!("Defined anonymous class: {:?}", new_class_id);
    // std::fs::write(format!("./anon_{}.class", new_class_id.get()), &data).unwrap();

    unsafe { env.get_local_jobject_for(static_form.into_generic()) }
}
