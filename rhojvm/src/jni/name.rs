/// Computes the native method name version of the method
/// Returns it with a null-terminator, because libloading says it can avoid an allocation that way
#[must_use]
pub fn make_native_method_name(class_name: &[u8], method_name: &[u8]) -> Vec<u8> {
    let mut result = b"Java_".to_vec();

    escape_name(class_name, &mut result);
    result.push(b'_');
    escape_name(method_name, &mut result);
    // FIXME: Descriptor for overloaded native method

    result.push(0);

    result
}

fn escape_name(name: &[u8], out: &mut Vec<u8>) {
    for ch in name {
        match *ch {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => out.push(*ch),
            b'/' => out.push(b'_'),
            b'_' => {
                out.push(b'_');
                out.push(b'1');
            }
            b';' => {
                out.push(b'_');
                out.push(b'2');
            }
            b'[' => {
                out.push(b'_');
                out.push(b'3');
            }
            _ => {
                // Encode ch as _0xxxx
                // TODO: This can be done without allocation
                let val = format!("{ch:04x}");
                out.push(b'_');
                out.push(b'0');
                for v in val.bytes() {
                    out.push(v);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::jni::name::make_native_method_name;

    #[test]
    fn test_native_method_name() {
        // TODO: More detailed tests
        assert_eq!(
            make_native_method_name(b"java/lang/System", b"registerNatives"),
            b"Java_java_lang_System_registerNatives\0"
        );

        assert_eq!(
            make_native_method_name(b"java/lang/invoke/MethodHandles$Lookup", b"findStatic"),
            b"Java_java_lang_invoke_MethodHandles_00024Lookup_findStatic\0"
        )
    }
}
