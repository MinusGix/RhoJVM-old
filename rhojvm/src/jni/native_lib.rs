use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
};

use libloading::Symbol;

use crate::jni;

#[derive(Debug)]
pub enum LoadLibraryError {
    LibLoading(libloading::Error),
}

#[derive(Debug)]
pub enum FindSymbolError {
    LibLoading(libloading::Error),
    FindFailure,
}
impl From<libloading::Error> for FindSymbolError {
    fn from(err: libloading::Error) -> FindSymbolError {
        FindSymbolError::LibLoading(err)
    }
}

pub struct NativeLibraries {
    libraries: HashMap<OsString, libloading::Library>,
}
impl NativeLibraries {
    #[must_use]
    pub fn new() -> NativeLibraries {
        NativeLibraries {
            libraries: HashMap::new(),
        }
    }

    /// Load a native library with a given name
    /// # Safety
    /// See [`libloading::Library::new`]
    /// This is very unsafe, and so primarily its safety relies on the JVM loading
    /// safe libraries that don't do absurd things like trounce over our memory.
    pub unsafe fn load_library(&mut self, path: impl AsRef<OsStr>) -> Result<(), LoadLibraryError> {
        let path = path.as_ref();
        if self.libraries.contains_key(path) {
            return Ok(());
        }

        tracing::info!("Loading Native Library '{:?}'", path);

        let lib = libloading::Library::new(path).map_err(LoadLibraryError::LibLoading)?;
        self.libraries.insert(path.to_owned(), lib);
        Ok(())
    }

    /// No mangling is done, you should likely get the name from
    /// [`jni::name::make_native_method_name`]
    /// If possible, include a null-byte at the end to avoid potential allocations, but is not req.
    /// Roughly `fn(*mut JNIEnv, JClass) -> void`
    /// # Safety
    /// The function specified must be of the correct function type for a JNI function
    /// that only takes a pointer to the environment and the static `JClass`.
    /// As well, the function itself must be safe, which is impossible to really guarantee.
    pub unsafe fn find_symbol_jni_static_nullary_void(
        &self,
        name: &[u8],
    ) -> Result<Symbol<jni::MethodClassNoArguments>, FindSymbolError> {
        for library in self.libraries.values() {
            let lib: Result<Symbol<jni::MethodClassNoArguments>, _> = library.get(name);
            if let Ok(lib) = lib {
                return Ok(lib);
            }
        }

        Err(FindSymbolError::FindFailure)
    }
}

impl Default for NativeLibraries {
    fn default() -> Self {
        Self::new()
    }
}
