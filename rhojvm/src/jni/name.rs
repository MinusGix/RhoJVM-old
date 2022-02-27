/// Computes the native method name version of the method
/// Returns it with a null-terminator, because libloading says it can avoid an allocation that way
#[must_use]
pub fn make_native_method_name(class_name: &[u8], method_name: &[u8]) -> Vec<u8> {
    let mut result = b"Java_".to_vec();

    escape_name(class_name).extend_vec(&mut result);
    result.push(b'_');
    escape_name(method_name).extend_vec(&mut result);
    // FIXME: Descriptor for overloded native method

    result.push(0);

    result
}
fn escape_name(name: &[u8]) -> EscapeNameIterator<'_> {
    EscapeNameIterator { name, index: 0 }
}

enum EscapeRes {
    Char(u8),
    TwoChar((u8, u8)),
}
impl EscapeRes {
    fn extend_vec(self, vec: &mut Vec<u8>) {
        match self {
            EscapeRes::Char(x) => vec.push(x),
            EscapeRes::TwoChar((x, y)) => {
                vec.push(x);
                vec.push(y);
            }
        }
    }
}
struct EscapeNameIterator<'a> {
    name: &'a [u8],
    index: usize,
}
impl<'a> EscapeNameIterator<'a> {
    fn extend_vec(self, vec: &mut Vec<u8>) {
        for res in self {
            res.extend_vec(vec);
        }
    }
}
impl<'a> Iterator for EscapeNameIterator<'a> {
    type Item = EscapeRes;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.name.get(self.index).map(|ch| match *ch {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => EscapeRes::Char(*ch),
            b'/' => EscapeRes::Char(b'_'),
            b'_' => EscapeRes::TwoChar((b'_', b'1')),
            b';' => EscapeRes::TwoChar((b'_', b'2')),
            b'[' => EscapeRes::TwoChar((b'_', b'3')),
            _ => todo!("UTF-16 code points aren't yet supported in native method names"),
        });

        if res.is_some() {
            self.index += 1;
        }

        res
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
    }
}
