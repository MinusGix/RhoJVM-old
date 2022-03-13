use std::collections::HashMap;

use rhojvm_base::id::ExactMethodId;

use crate::jni::OpaqueClassMethod;

#[derive(Clone, Default)]
pub struct MethodInfo {
    methods: HashMap<ExactMethodId, MethodData>,
}
impl MethodInfo {
    #[must_use]
    pub fn get(&self, id: ExactMethodId) -> Option<&MethodData> {
        self.methods.get(&id)
    }

    #[must_use]
    pub fn get_mut(&mut self, id: ExactMethodId) -> Option<&mut MethodData> {
        self.methods.get_mut(&id)
    }

    #[must_use]
    pub fn get_init(&mut self, id: ExactMethodId) -> &MethodData {
        self.methods
            .entry(id)
            .or_insert_with(|| MethodData::new(id))
    }

    #[must_use]
    pub fn get_mut_init(&mut self, id: ExactMethodId) -> &mut MethodData {
        self.methods
            .entry(id)
            .or_insert_with(|| MethodData::new(id))
    }

    /// Initialize [`MethodData`] if it doesn't exist
    /// Then passes it into given function for further modification
    /// It is inserted into `methods` before the function is called
    pub fn modify_init_with<F: FnOnce(&mut MethodData)>(&mut self, id: ExactMethodId, f: F) {
        let data = self.get_mut_init(id);
        f(data);
    }
}

#[non_exhaustive]
#[derive(Clone)]
pub enum NativeMethod {
    /// An opaque method found by the symbol's name
    /// Should only be used for `native` methods
    OpaqueFound(OpaqueClassMethod),
    /// An opaque method registered by a call to `RegisterNatives`
    /// /// Should only be used for `native` methods
    OpaqueRegistered(OpaqueClassMethod),
    // TODO: Variants for like jitted methods or overrides
}
impl NativeMethod {
    #[must_use]
    pub fn get(&self) -> &OpaqueClassMethod {
        match self {
            NativeMethod::OpaqueFound(x) | NativeMethod::OpaqueRegistered(x) => x,
        }
    }
}

#[derive(Clone)]
pub struct MethodData {
    id: ExactMethodId,
    /// A native function that should be called in place of the method body
    pub native_func: Option<NativeMethod>,
}
impl MethodData {
    pub(crate) fn new(id: ExactMethodId) -> MethodData {
        MethodData {
            id,
            native_func: None,
        }
    }

    #[must_use]
    pub fn id(&self) -> ExactMethodId {
        self.id
    }
}
