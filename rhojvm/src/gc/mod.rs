//! This is currently a rather basic implementation of a Garbage Collector
//! It could be improved, especially if we take advantage of platform specific optimizations
//! such as using higher bits on pointers on 64-bit platforms.
//! A notable goal for this module is to allow it to be some degree of swappable.
//! The optimal state of things is to allow the usage of different garbage collector schemes
//! at runtime.
//! However, it is also desired that that if one wishes to compile an instance of this library with
//! a specific GC then it would be possible. (Such as to allow optimizations for the case where the
//! compiler can see through function pointers).
//!
//! Currently, this is an early implementation of the gc library and so it would not be overly
//! surprising if it needed to be completely reworked.
//! An issue this has is that it is not _just_ for handling JVM references, but we also wish
//! to allow usage from Rust itself.
//! This initial implementation is very much inspired by:
//! <https://github.com/ceronman/loxido/blob/master/src/gc.rs>
use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use rhojvm_base::{
    data::{class_files::ClassFiles, class_names::ClassNames, classes::Classes, methods::Methods},
    package::Packages,
    util::MemorySize,
};

use crate::{
    class_instance::{Instance, ReferenceInstance},
    rv::RuntimeValue,
    util, State,
};

// TODO: make GcRef hold a reference count, so then we can keep track of Rust losing the values?
// TODO: We could make this just hold specific types, to make it easier for the tracing to be
// optimized
// Since for the most part, we're only storing class instances
// TODO: We could have a quiet_objects vector which keeps track of objects which can never
// hold anything and so don't have any use in being traced.
// TODO: We could have a string interner so we reuse strings
pub struct Gc {
    /// The rough amount of memory that the objects are using
    bytes_used: usize,
    /// The amount of memory that should be used before our next garbage collection
    next_gc: usize,
    /// The allocated objects, with various spots being empty
    objects: Vec<Option<GcObject>>,
    /// Free slots, being in a vector so that it is faster to get one as we need it
    free_slots: Vec<usize>,
    grey_stack: VecDeque<usize>,
}
impl Gc {
    const HEAP_GROW_FACTOR: usize = 2;

    #[must_use]
    pub fn new() -> Gc {
        Gc {
            bytes_used: 0,
            next_gc: 1024 * 1024,
            objects: Vec::new(),
            free_slots: Vec::new(),
            grey_stack: VecDeque::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &'_ GcObject> {
        self.objects.iter().filter_map(Option::as_ref)
    }

    pub fn iter_ref(&self) -> impl Iterator<Item = (GcRef<Instance>, &'_ GcObject)> {
        self.objects
            .iter()
            .enumerate()
            .map(|(i, obj)| (GcRef::new_unchecked(i), obj))
            .filter_map(|(i, x)| x.as_ref().map(|x| (i, x)))
    }

    pub fn alloc<'a, T: Into<Instance> + 'static>(&'a mut self, value: T) -> GcRef<T>
    where
        &'a T: TryFrom<&'a Instance>,
        &'a mut T: TryFrom<&'a mut Instance>,
    {
        let value = value.into();
        // The amount of memory that the value and our tracking will take in memory
        let memory_size = value.memory_size() + std::mem::size_of::<GcObject>();
        // TODO: Checked add in case it tries allocating something absurd
        self.bytes_used += memory_size;

        let object = GcObject {
            marked: false,
            size: memory_size,
            value,
        };

        // Find a free spot, if there is any, for us to use
        let index = if let Some(i) = self.free_slots.pop() {
            debug_assert!(self.objects[i].is_none());
            self.objects[i] = Some(object);
            i
        } else {
            // There was no free slot so we have to push
            self.objects.push(Some(object));
            self.objects.len() - 1
        };

        GcRef::new_unchecked(index)
    }

    /// Clones a given reference shallowly, returning a new `GcRef` to it
    /// Since this is shallow, this does mean that any references held inside will be the same
    /// in the cloned version.
    /// Note that we don't actually use the `T` parameter, it is simply to make the `GcRef` api
    /// easier
    /// If it exists, but the `T` parameter is incorrect, it will still be shallowly cloned,
    /// but the returned `GcRef<T>` will also be bad too.
    /// Remember that unless you store it somewhere, the gc might pick it up.
    pub fn shallow_clone<T>(&mut self, reference: GcRef<T>) -> Option<GcRef<T>> {
        let object = self.objects.get(reference.index).and_then(Option::as_ref)?;
        let object = GcObject {
            marked: false,
            size: object.size,
            value: object.value.clone(),
        };
        self.bytes_used += object.size;

        let index = if let Some(i) = self.free_slots.pop() {
            debug_assert!(self.objects[i].is_none());
            self.objects[i] = Some(object);
            i
        } else {
            self.objects.push(Some(object));
            self.objects.len() - 1
        };

        Some(GcRef::new_unchecked(index))
    }

    /// Converts a `GcRef<T>` to a `GcRef<U>`, if the `U` type can be deref'd into an value.
    /// If `GcRef<T>` can't be deref'd then it returns a similarly-incorrect `GcRef<U>`.
    pub fn checked_as<'a, T: 'static, U: 'static>(&'a self, reference: GcRef<T>) -> Option<GcRef<U>>
    where
        &'a T: TryFrom<&'a Instance>,
        &'a U: TryFrom<&'a Instance>,
    {
        let Some(obj) = self.objects.get(reference.index).and_then(Option::as_ref) else {
            // The reference doesn't exist, so we just let the bad reference through
            return Some(reference.unchecked_as())
        };

        if <&U>::try_from(&obj.value).is_ok() {
            Some(reference.unchecked_as())
        } else {
            // Failed to convert
            None
        }
    }

    #[must_use]
    pub fn deref<'a, T>(&'a self, reference: GcRef<T>) -> Option<&'a T>
    where
        &'a T: TryFrom<&'a Instance>,
    {
        self.objects
            .get(reference.index)
            .and_then(Option::as_ref)
            .map(|obj| &obj.value)
            .and_then(|obj| <&T>::try_from(obj).ok())
    }

    #[must_use]
    pub fn deref_mut<'a, T>(&'a mut self, reference: GcRef<T>) -> Option<&'a mut T>
    where
        &'a mut T: TryFrom<&'a mut Instance>,
    {
        self.objects
            .get_mut(reference.index)
            .and_then(Option::as_mut)
            .map(|obj| &mut obj.value)
            .and_then(|obj| <&mut T>::try_from(obj).ok())
    }

    #[must_use]
    pub fn deref_disjoint2_mut<'a, T, U>(
        &'a mut self,
        ref1: GcRef<T>,
        ref2: GcRef<U>,
    ) -> Option<(&'a mut T, &'a mut U)>
    where
        &'a mut T: TryFrom<&'a mut Instance>,
        &'a mut U: TryFrom<&'a mut Instance>,
    {
        let (val1, val2) = util::get_disjoint2_mut(&mut self.objects, ref1.index, ref2.index)?;
        let val1 = &mut val1.as_mut()?.value;
        let val1 = <&mut T>::try_from(val1).ok()?;
        let val2 = &mut val2.as_mut()?.value;
        let val2 = <&mut U>::try_from(val2).ok()?;

        Some((val1, val2))
    }

    pub fn mark_object(&mut self, obj: GcRef<Instance>) {
        if let Some(object) = self.objects.get_mut(obj.index).and_then(Option::as_mut) {
            if object.marked {
                return;
            }

            object.marked = true;
            self.grey_stack.push_back(obj.index);
        } else {
            debug_assert!(false, "Marking already disposed of object {}", obj.index);
        }
    }

    #[must_use]
    pub fn should_gc(&self) -> bool {
        self.bytes_used > self.next_gc
    }

    pub fn collect_garbage(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        classes: &mut Classes,
        packages: &mut Packages,
        methods: &mut Methods,
        state: &mut State,
    ) {
        self.trace_references(class_names, class_files, classes, packages, methods, state);
        // self.remove_white_strings()
        self.sweep();
        self.next_gc = self.bytes_used * Gc::HEAP_GROW_FACTOR;
    }

    fn trace_references(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        classes: &mut Classes,
        packages: &mut Packages,
        methods: &mut Methods,
        state: &mut State,
    ) {
        while let Some(index) = self.grey_stack.pop_back() {
            self.blacken_object(
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                index,
            );
        }
    }

    // TODO: Free should perform the finalization infrastructure that the JVM has
    // because objects can do things on finalization, such as cleanup or even rejuvenating
    // themselves.
    // I'm honestly not sure how to implement the finalization without doing multiple Gc scans until
    // nothing changes
    /// Should always receive a valid index into the objects vector
    /// May panic if the index is invalid
    fn free(&mut self, index: usize) {
        if let Some(old) = self.objects[index].take() {
            self.bytes_used -= old.size;
            self.free_slots.push(index);
        } else {
            debug_assert!(false, "Double free on {}", index);
        }
    }

    /// Should always receive a valid index into the objects vector
    /// May panic if the index is invalid
    fn blacken_object(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        classes: &mut Classes,
        packages: &mut Packages,
        methods: &mut Methods,
        state: &mut State,
        index: usize,
    ) {
        let reference = GcRef::new_unchecked(index);
        trace_instance(
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            self,
            reference,
        );
    }

    fn sweep(&mut self) {
        for i in 0..self.objects.len() {
            if let Some(mut object) = self.objects[i].as_mut() {
                if object.marked {
                    object.marked = false;
                } else {
                    self.free(i);
                }
            }
        }
    }

    fn mark(&mut self, obj: GcRef<Instance>) {
        if let Some(object) = self.objects.get_mut(obj.index).and_then(Option::as_mut) {
            if object.marked {
                // It was already seen
                return;
            }

            object.marked = true;
            self.grey_stack.push_back(obj.index);
        }
    }
}

impl Default for Gc {
    fn default() -> Self {
        Self::new()
    }
}

pub struct GcObject {
    marked: bool,
    size: usize,
    value: Instance,
}
impl GcObject {
    #[must_use]
    pub fn value(&self) -> &Instance {
        &self.value
    }
}

/// Marks that a value is stored on the heap
/// This is mostly to make constraints on various type safe parts of the code easier
pub trait GcValueMarker {}

// TODO: We could shrink index, because a 64-bit platform doesn't really need to hold that many
// unique objects.. nor can we even store that many in a Rust vector.
// We could use that extra space as a generation id, to make so if the native code is unsound
//  (or our code using the native code is unsound in terms of JVM guarantees)
// which would then help avoid behaving badly.
/// A reference to an object in the Gc
/// Should not be used across Gc instances
pub struct GcRef<T> {
    index: usize,
    _marker: PhantomData<T>,
}
impl<T> GcRef<T> {
    pub(crate) fn new_unchecked(index: usize) -> GcRef<T> {
        GcRef {
            index,
            _marker: PhantomData,
        }
    }

    pub(crate) fn get_index_unchecked(self) -> usize {
        self.index
    }

    // TODO: These constraints are probably not as exacting as they could be
    /// Convert reference into more generic instance of the type
    #[must_use]
    pub fn into_generic<U>(self) -> GcRef<U>
    where
        U: From<T> + GcValueMarker,
    {
        GcRef {
            index: self.index,
            _marker: PhantomData,
        }
    }

    /// Converts the generic parameter into U, unchecked  
    /// This does not check that the `GcRef` would actually work for that type. If you want that,
    /// then use [`Gc::checked_as`]
    #[must_use]
    pub fn unchecked_as<U>(self) -> GcRef<U> {
        GcRef {
            index: self.index,
            _marker: PhantomData,
        }
    }

    /// Converts a `GcRef<T>` to a `GcRef<U>`, if the `U` type can be deref'd into an value.
    /// If `GcRef<T>` can't be deref'd then it returns a similarly-incorrect `GcRef<U>`.
    pub fn checked_as<'a, U>(&self, gc: &'a Gc) -> Option<GcRef<U>>
    where
        T: 'static,
        U: 'static,
        &'a T: TryFrom<&'a Instance>,
        &'a U: TryFrom<&'a Instance>,
    {
        gc.checked_as(*self)
    }
}
impl<T> Copy for GcRef<T> {}
impl<T> Clone for GcRef<T> {
    #[inline]
    fn clone(&self) -> GcRef<T> {
        *self
    }
}

// This can be wrong if there is more than one Gc instance
impl<T> Eq for GcRef<T> {}
impl<T> PartialEq for GcRef<T> {
    fn eq(&self, other: &GcRef<T>) -> bool {
        self.index == other.index
    }
}

impl<T> Hash for GcRef<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T> std::fmt::Debug for GcRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let full_name = std::any::type_name::<T>();
        // Get the last part if possible, otherwise use the full name
        let name = full_name.split("::").last().unwrap_or(full_name);
        write!(f, "gcref({}:{})", self.index, name)
    }
}

/// A standalone function to trace a reference
/// This is separate from `Instance` itself, and the `Gc`, because we may need to
/// do arbitrary things with the `Gc`, which may invalidate any reference we would get.
/// There is also the issue that if we decided to just clone the `Instance` we are tracing
///    (which isn't cheap)
/// we may ignore changes to the data.
///
/// This can assume that the ref is valid at first, but it should be careful after running code.
fn trace_instance(
    _class_names: &mut ClassNames,
    _class_files: &mut ClassFiles,
    _classes: &mut Classes,
    _packages: &mut Packages,
    _methods: &mut Methods,
    _state: &mut State,
    gc: &mut Gc,
    instance_ref: GcRef<Instance>,
) -> Option<()> {
    let instance = if let Some(instance) = gc.deref(instance_ref) {
        instance
    } else {
        tracing::warn!(
            "GC: Ref {:?} was not found, it may have already been freed",
            instance_ref
        );
        return None;
    };
    // TODO: Can we avoid allocating to a vector, or at least use a small vec since many classes
    // won't have a large number of fields (maybe a dozen would be a good amount?)
    let fields = instance
        .fields()
        .map(|x| x.1.value())
        .filter_map(|x| match x {
            RuntimeValue::Reference(val_ref) => Some(val_ref),
            _ => None,
        })
        .collect::<Vec<_>>();
    for field_value_ref in fields {
        gc.mark(field_value_ref.into_generic());
    }

    let instance = gc.deref(instance_ref).unwrap();
    match instance {
        Instance::Reference(refe) => match refe {
            ReferenceInstance::Class(class) => {
                let x = class.static_ref;
                gc.mark(x.into_generic());
            }
            ReferenceInstance::StaticForm(class) => {
                let class_ref = class.inner.static_ref;
                // let held_ref = class.of;
                gc.mark(class_ref.into_generic());
                // if let Some(held_ref) = held_ref {
                //     gc.mark(held_ref.into_generic());
                // }
            }
            ReferenceInstance::Thread(thread) => {
                let class_ref = thread.inner.static_ref;
                gc.mark(class_ref.into_generic());
            }
            ReferenceInstance::MethodHandle(_handle) => {
                // TODO:
            }
            ReferenceInstance::MethodHandleInfo(_handle_info) => {
                // TODO:
            }
            ReferenceInstance::PrimitiveArray(_) => (),
            ReferenceInstance::ReferenceArray(array) => {
                // TODO: It would be great if we could avoid allocating here
                let elements = array.elements.clone();
                for element in elements.into_iter().flatten() {
                    gc.mark(element.into_generic());
                }
            }
        },
        Instance::StaticClass(_) => (),
    }

    Some(())
}
