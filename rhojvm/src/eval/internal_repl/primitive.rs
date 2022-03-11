use rhojvm_base::code::types::JavaChar;

use crate::{
    eval::ValueException,
    jni::{JDouble, JFloat, JInt, JLong, JObject, JString},
    rv::RuntimeValuePrimitive,
    util::{self, Env},
};

pub(crate) extern "C" fn float_to_raw_int_bits(
    _env: *mut Env<'_>,
    _this: JObject,
    value: JFloat,
) -> JInt {
    i32::from_be_bytes(value.to_be_bytes())
}

pub(crate) extern "C" fn double_to_raw_long_bits(
    _env: *mut Env<'_>,
    _this: JObject,
    value: JDouble,
) -> JLong {
    i64::from_be_bytes(value.to_be_bytes())
}

// TODO: Is this correct for hex/binary/octal in java's integer class?
pub(crate) extern "C" fn integer_to_string(
    env: *mut Env<'_>,
    _this: JObject,
    val: JInt,
    radix: JInt,
) -> JString {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    if !(2..=36).contains(&radix) {
        todo!("Exception, radix was out of bounds");
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let radix = radix as u8;

    let result = radix_fmt::radix(val, radix as u8);
    // java uses lowercase for this
    let result = format!("{}", result);
    let result = result
        .encode_utf16()
        .map(JavaChar)
        .map(RuntimeValuePrimitive::Char)
        .collect::<Vec<_>>();

    let string = util::construct_string(env, result).expect("Failed to create string");
    let string = match string {
        ValueException::Value(string) => string,
        ValueException::Exception(_) => {
            todo!("There was an exception converting integer to string")
        }
    };

    unsafe { env.get_local_jobject_for(string.into_generic()) }
}

pub(crate) extern "C" fn integer_parse_int(
    env: *mut Env<'_>,
    _this: JObject,
    source: JString,
    radix: JInt,
) -> JInt {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    if !(2..=36).contains(&radix) {
        todo!("Exception, radix was out of bounds");
    }

    let radix = radix.unsigned_abs();

    let source = unsafe { env.get_jobject_as_gcref(source) };
    let source = source.expect("null source ref");
    let source = util::get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        source,
    )
    .unwrap();

    // TODO: We could do this manually ourselves directly from the utf16 string, which would be
    // faster than converting it to a rust string and back..
    // TODO: Does this match java's behavior?
    i32::from_str_radix(&source, radix).expect("Failed to parse integer")
}
