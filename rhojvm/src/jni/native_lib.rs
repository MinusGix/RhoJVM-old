use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    sync::RwLock,
};

use libloading::Symbol;

use crate::{eval::internal_repl::find_internal_rho_native_method, jni};

use super::{JNIOnLoadFn, OpaqueClassMethod};

#[derive(Debug)]
pub enum LoadLibraryError {
    LibLoading(libloading::Error),
}

#[derive(Debug)]
pub enum FindSymbolError {
    LibLoading(libloading::Error),
    /// The symbol was a null pointer
    NullSymbol,
    FindFailure,
    BadLibraryPath,
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

    /// Find a given symbol for a function
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

// I wish libloading exported these types
#[cfg(unix)]
pub type StaticSymbol<T> = libloading::os::unix::Symbol<T>;
#[cfg(windows)]
pub type StaticSymbol<T> = libloading::os::windows::Symbol<T>;

/// Version of native libraries that holds a static reference and does locking
/// Returning non-lifetime versions of symbols _should_ be fine since a box
/// holding a [`NativeLibraries`] is leaked, so that it will never disappear.
pub struct NativeLibrariesStatic {
    // TODO: We could use Dashmap?
    lib: RwLock<&'static mut NativeLibraries>,
}
impl NativeLibrariesStatic {
    #[must_use]
    pub fn new() -> NativeLibrariesStatic {
        NativeLibrariesStatic {
            lib: RwLock::new(Box::leak(Box::new(NativeLibraries::new()))),
        }
    }

    /// Load a native library with a given name
    /// Blocks current thread until it is given access
    /// # Safety
    /// See [`libloading::Library::new`]
    /// This is very unsafe, and so primarily its safety will rely on the JVM loading safe libraries
    /// that don't do absurd things like trounce over our memory.
    /// # Panics
    /// May panic if the lock is already held by the current thread
    /// May panic if the lock is poisoned
    pub unsafe fn load_library_blocking(
        &self,
        path: impl AsRef<OsStr>,
    ) -> Result<(), LoadLibraryError> {
        let mut lib = self.lib.write().expect("Native Library lock was poisoned");
        lib.load_library(path)?;
        Ok(())
    }

    /// Find a given symbol for a function
    /// No mangling is done, you should likely get the name from
    /// [`jni::name::make_native_method_name`]
    /// If possible, include a null-byte at the end to avoid potential allocations, but is not req.
    /// Roughly `fn(*mut JNIEnv, JClass) -> void`
    /// Blocks current thread until it is given access
    /// # Safety
    /// The function must be a valid function, but does not necessarily have
    /// The function specified must be of the correct function type for a JNI function
    /// that also takes some [`JObject`] (or type which can be treated as it)
    /// It may also have more parameters, and so the safety of calling it depends on that.
    /// Calling the returned function would also require that the function itself is safe,
    /// but that is practically impossible to guarantee.
    /// # Panics
    /// May panic if the lock is already held by the current thread
    /// May panic if the lock is poisoned
    pub unsafe fn find_symbol_blocking_jni_opaque_method(
        &self,
        name: &[u8],
    ) -> Result<OpaqueClassMethod, FindSymbolError> {
        if let Some(internal_method) = find_internal_rho_native_method(name) {
            Ok(internal_method)
        } else {
            let symbol = {
                let lib = self.lib.read().expect("Native Library lock was poisoned");
                let symbol: Symbol<jni::MethodClassNoArguments> =
                    lib.find_symbol_jni_static_nullary_void(name)?;
                let symbol: StaticSymbol<jni::MethodClassNoArguments> = symbol.into_raw();
                symbol
            };
            let symbol = symbol.into_raw();
            if symbol.is_null() {
                return Err(FindSymbolError::NullSymbol);
            }

            // Transmute the raw pointer into a function pointer
            // Safety: We've already checked if it was null, and if it was then we returned an error.
            // We also provided the same guarantees/info to libloading, not that it pays attention to
            // that.
            let symbol =
                std::mem::transmute::<*mut std::ffi::c_void, jni::MethodClassNoArguments>(symbol);

            Ok(OpaqueClassMethod::new(symbol))
        }
    }

    /// Find a [`JNI_OnLoad`] function if one exists
    /// # Safety
    /// The function must be a valid function
    /// The safety of calling it primarily depends on the code itself
    pub(crate) unsafe fn find_symbol_blocking_jni_on_load_in_library(
        &self,
        lib_path: impl AsRef<OsStr>,
    ) -> Result<JNIOnLoadFn, FindSymbolError> {
        let symbol = {
            let native_lib = self.lib.read().expect("Native Library lock was poisoned");
            let library = native_lib
                .libraries
                .get(lib_path.as_ref())
                .ok_or(FindSymbolError::BadLibraryPath)?;
            let symbol: Symbol<JNIOnLoadFn> = library
                .get(b"JNI_OnLoad")
                .map_err(|_| FindSymbolError::FindFailure)?;
            let symbol: StaticSymbol<JNIOnLoadFn> = symbol.into_raw();
            symbol
        };
        let symbol = symbol.into_raw();
        if symbol.is_null() {
            return Err(FindSymbolError::NullSymbol);
        }

        // Transmute the raw pointer into a function pointer
        // Safety: We've already checked if it was null, and if it was then we returned an error.
        // We also provided the same guarantees/info to libloading, not that it pays attention to
        // that.
        let symbol = std::mem::transmute::<*mut std::ffi::c_void, JNIOnLoadFn>(symbol);

        Ok(symbol)
    }
}

impl Default for NativeLibrariesStatic {
    fn default() -> Self {
        Self::new()
    }
}
