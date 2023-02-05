use rhojvm_base::code::types::JavaChar;

use crate::{
    eval::{internal_repl::JINT_GARBAGE, ValueException},
    jni::{JClass, JDouble, JFloat, JInt, JLong, JObject, JString},
    rv::RuntimeValuePrimitive,
    util::{self, make_exception_by_name, Env},
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

// int numberOfLeadingZeros(long value);
pub(crate) extern "C" fn long_number_of_leading_zeroes(
    _env: *mut Env<'_>,
    _this: JClass,
    value: JLong,
) -> JInt {
    value.leading_zeros() as i32
}

// TODO: Is this correct for hex/binary/octal in java's long class?
pub(crate) extern "C" fn long_to_string(
    env: *mut Env<'_>,
    _this: JObject,
    val: JLong,
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

pub(crate) extern "C" fn long_parse_int(
    env: *mut Env<'_>,
    _this: JObject,
    source: JString,
    radix: JInt,
) -> JLong {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    if !(2..=36).contains(&radix) {
        todo!("Exception, radix was out of bounds");
    }

    let radix = radix.unsigned_abs();

    let source = unsafe { env.get_jobject_as_gcref(source) };
    let source = if let Some(source) = source {
        source
    } else {
        let npe = make_exception_by_name(
            env,
            b"java/lang/NullPointerException",
            "Integer#parseInt source was null",
        )
        .expect("Failed to create NPE")
        .flatten();
        env.state.fill_native_exception(npe);

        // There is no meaningful return value
        return JINT_GARBAGE.into();
    };
    let source = util::get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        source,
    );
    let source = match source {
        Ok(source) => source,
        Err(_) => todo!(),
    };

    // TODO: We could do this manually ourselves directly from the utf16 string, which would be
    // faster than converting it to a rust string and back..
    // TODO: Does this match java's behavior?
    match i64::from_str_radix(&source, radix) {
        Ok(value) => value,
        Err(err) => {
            let err_text = format!("{}", err);

            let exc = make_exception_by_name(env, b"java/lang/NumberFormatException", &err_text)
                .expect("Failed to create exception")
                .flatten();
            env.state.fill_native_exception(exc);

            JINT_GARBAGE.into()
        }
    }
}

// int numberOfLeadingZeros(int value);
pub(crate) extern "C" fn integer_number_of_leading_zeroes(
    _env: *mut Env<'_>,
    _this: JClass,
    value: JInt,
) -> JInt {
    value.leading_zeros() as i32
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
    let source = if let Some(source) = source {
        source
    } else {
        let npe = make_exception_by_name(
            env,
            b"java/lang/NullPointerException",
            "Integer#parseInt source was null",
        )
        .expect("Failed to create NPE")
        .flatten();
        env.state.fill_native_exception(npe);

        // There is no meaningful return value
        return JINT_GARBAGE;
    };
    let source = util::get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        source,
    );
    let source = match source {
        Ok(source) => source,
        Err(_) => todo!(),
    };

    // TODO: We could do this manually ourselves directly from the utf16 string, which would be
    // faster than converting it to a rust string and back..
    // TODO: Does this match java's behavior?
    match i32::from_str_radix(&source, radix) {
        Ok(value) => value,
        Err(err) => {
            let err_text = format!("{}", err);

            let exc = make_exception_by_name(env, b"java/lang/NumberFormatException", &err_text)
                .expect("Failed to create exception")
                .flatten();
            env.state.fill_native_exception(exc);

            JINT_GARBAGE
        }
    }
}
