use std::{collections::HashMap, hash::BuildHasherDefault, num::NonZeroUsize};

use classfile_parser::{
    constant_info::{ClassConstant, Utf8Constant},
    constant_pool::ConstantPoolIndexRaw,
    ClassAccessFlags,
};
use smallvec::SmallVec;

use crate::{
    class::{ArrayClass, ArrayComponentType, Class, ClassFileIndexError, ClassVariant},
    code::{
        method::{DescriptorType, DescriptorTypeBasic},
        types::PrimitiveType,
    },
    id::ClassId,
    package::Packages,
    util::{self},
    BadIdError, StepError,
};

use super::{
    class_file_loader::LoadClassFileError,
    class_files::{ClassFiles, SuperClassFileIterator},
    class_names::ClassNames,
};

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadClassError {
    BadId(BadIdError),
    LoadClassFile(LoadClassFileError),
    ClassFileIndex(ClassFileIndexError),
    /// An invalid index into the constant pool for an interface
    BadInterfaceIndex(ConstantPoolIndexRaw<ClassConstant>),
    /// An invalid index for an interface's name into the constant pool
    BadInterfaceNameIndex(ConstantPoolIndexRaw<Utf8Constant>),
}
impl From<ClassFileIndexError> for LoadClassError {
    fn from(err: ClassFileIndexError) -> Self {
        Self::ClassFileIndex(err)
    }
}

#[derive(Debug, Default, Clone)]
pub struct Classes {
    /// Whether to log that we're loading a class
    /// Uses `tracing::info!`
    pub log_load: bool,
    map: HashMap<
        ClassId,
        ClassVariant,
        <util::HashWrapper as util::HashWrapperTrait<ClassId>>::HashMapHasher,
    >,
}
impl Classes {
    #[must_use]
    pub fn new() -> Classes {
        Classes {
            log_load: false,
            map: HashMap::with_hasher(BuildHasherDefault::default()),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[must_use]
    pub fn contains_key(&self, key: &ClassId) -> bool {
        self.map.contains_key(key)
    }

    #[must_use]
    pub fn get(&self, key: &ClassId) -> Option<&ClassVariant> {
        self.map.get(key)
    }

    #[must_use]
    pub fn get_mut(&mut self, key: &ClassId) -> Option<&mut ClassVariant> {
        self.map.get_mut(key)
    }

    pub(crate) fn set_at(&mut self, key: ClassId, val: ClassVariant) {
        if self.map.insert(key, val).is_some() {
            tracing::warn!("Duplicate setting for Classes with {:?}", key);
            debug_assert!(false);
        }
    }

    // FIXME: This doesn't force any verification
    /// The given array class must have valid and correct fields!
    pub fn register_array_class(&mut self, array_class: ArrayClass) {
        self.set_at(array_class.id(), ClassVariant::Array(array_class));
    }

    pub fn load_class(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_file_id: ClassId,
    ) -> Result<(), StepError> {
        if self.contains_key(&class_file_id) {
            // It was already loaded
            return Ok(());
        }

        let (_, class_info) = class_names
            .name_from_gcid(class_file_id)
            .map_err(StepError::BadId)?;
        if self.log_load {
            tracing::info!("====> C{:?}", class_names.tpath(class_file_id));
        }

        if !class_info.has_class_file() {
            // Just load the array class
            self.get_array_class(class_names, class_files, packages, class_file_id)?;
            return Ok(());
        }

        // Requires the class file to be loaded
        if !class_files.contains_key(&class_file_id) {
            class_files.load_by_class_path_id(class_names, class_file_id)?;
        }

        let class_file = class_files.get(&class_file_id).unwrap();

        let this_class_name = class_file
            .get_this_class_name()
            .map_err(LoadClassError::ClassFileIndex)?;
        let super_class_id = class_file
            .get_super_class_id(class_names)
            .map_err(LoadClassError::ClassFileIndex)?;

        let package = util::access_path_initial_part(this_class_name);
        let package = package.map(|package| packages.slice_path_create_if_needed(package));

        let class = Class::new(
            class_file_id,
            super_class_id,
            package,
            class_file.access_flags(),
            class_file.methods_len(),
        );

        self.set_at(class_file_id, ClassVariant::Class(class));

        Ok(())
    }

    // TODO: We could maybe generate the id for these various arrays without string
    // allocations so that we can simply check if they exist cheaply
    pub fn load_array_of_instances(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_id: ClassId,
    ) -> Result<ClassId, StepError> {
        let component_type = ArrayComponentType::Class(class_id);

        let id = class_names
            .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), class_id)
            .map_err(StepError::BadId)?;
        if let Some(class) = self.get(&id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(id);
        }

        let (package, access_flags) = {
            // TODO: For normal classes, we only need to load the class file
            self.load_class(class_names, class_files, packages, class_id)?;
            let class = self.get(&class_id).unwrap();
            (class.package(), class.access_flags())
        };
        let array = ArrayClass {
            id,
            super_class: class_names.object_id(),
            component_type,
            access_flags,
            package,
        };
        self.register_array_class(array);
        Ok(id)
    }

    pub fn load_array_of_primitives(
        &mut self,
        class_names: &mut ClassNames,
        prim: PrimitiveType,
    ) -> Result<ClassId, StepError> {
        let component_type = ArrayComponentType::from(prim);

        let array_id = class_names.gcid_from_array_of_primitives(prim);
        if let Some(class) = self.get(&array_id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(array_id);
        }

        let array = ArrayClass::new_unchecked(
            array_id,
            component_type,
            class_names.object_id(),
            // Since all the types are primitive, we can simply use this
            ClassAccessFlags::PUBLIC,
            None,
        );
        self.register_array_class(array);
        Ok(array_id)
    }

    pub fn load_level_array_of_primitives(
        &mut self,
        class_names: &mut ClassNames,
        level: NonZeroUsize,
        prim: PrimitiveType,
    ) -> Result<ClassId, StepError> {
        let array_id = class_names.gcid_from_level_array_of_primitives(level, prim);
        if let Some(class) = self.get(&array_id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(array_id);
        }

        // If level > 1 then the component type isn't the above component type, but rather
        // another array.
        // We don't bother registering it, only registering the name
        let component_type =
            if let Some(level) = level.get().checked_sub(1).and_then(NonZeroUsize::new) {
                let component_id = class_names.gcid_from_level_array_of_primitives(level, prim);
                ArrayComponentType::Class(component_id)
            } else {
                prim.into()
            };

        let array = ArrayClass::new_unchecked(
            array_id,
            component_type,
            class_names.object_id(),
            ClassAccessFlags::PUBLIC,
            None,
        );
        self.register_array_class(array);

        Ok(array_id)
    }

    /// Load an array of the given descriptor type
    pub fn load_level_array_of_desc_type(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        level: NonZeroUsize,
        component: DescriptorType,
    ) -> Result<ClassId, StepError> {
        match component {
            DescriptorType::Basic(b) => self.load_level_array_of_desc_type_basic(
                class_names,
                class_files,
                packages,
                level,
                b,
            ),
            DescriptorType::Array { level, component } => {
                let component_id = self.load_level_array_of_desc_type_basic(
                    class_names,
                    class_files,
                    packages,
                    level,
                    component,
                )?;
                self.load_array_of_instances(class_names, class_files, packages, component_id)
            }
        }
    }

    pub fn load_level_array_of_desc_type_basic(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        level: NonZeroUsize,
        component: DescriptorTypeBasic,
    ) -> Result<ClassId, StepError> {
        let array_id = class_names
            .gcid_from_level_array_of_desc_type_basic(level, component)
            .map_err(StepError::BadId)?;
        if let Some(class) = self.get(&array_id) {
            // It was already loaded
            debug_assert!(matches!(class, ClassVariant::Array(_)));
            return Ok(array_id);
        }

        let component_id =
            load_basic_descriptor_type(self, class_names, class_files, packages, component)?;

        let (package, access_flags) = if let Some(component_id) = component_id {
            // TODO: For normal classes, we only need to load the class file
            self.load_class(class_names, class_files, packages, component_id)?;
            let class = self.get(&component_id).unwrap();
            (class.package(), class.access_flags())
        } else {
            // These methods only return none if it was a class, but if it was then it would
            // be in the other branch
            (None, component.access_flags().unwrap())
        };

        // If level > 1 then the component type isn't the above component type, but rather
        // another array.
        // We don't bother registering it, only registering the name
        let component_type =
            if let Some(level) = level.get().checked_sub(1).and_then(NonZeroUsize::new) {
                let component_id = class_names
                    .gcid_from_level_array_of_desc_type_basic(level, component)
                    .map_err(StepError::BadId)?;
                ArrayComponentType::Class(component_id)
            } else {
                component.as_array_component_type()
            };

        let array = ArrayClass {
            id: array_id,
            super_class: class_names.object_id(),
            component_type,
            access_flags,
            package,
        };
        self.register_array_class(array);

        Ok(array_id)
    }

    pub fn load_level_array_of_class_id(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        level: NonZeroUsize,
        class_id: ClassId,
    ) -> Result<ClassId, StepError> {
        // TODO: Inline this so that we do slightly less work
        self.load_level_array_of_desc_type_basic(
            class_names,
            class_files,
            packages,
            level,
            DescriptorTypeBasic::Class(class_id),
        )
    }

    /// Returns the [`ArrayClass`] if it is an array
    /// This should be used rather than loading the class itself, because this
    /// avoids loading classes that it doesn't need to.
    pub fn get_array_class(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_id: ClassId,
    ) -> Result<Option<&ArrayClass>, StepError> {
        use classfile_parser::descriptor::DescriptorType as DescriptorTypeCF;

        // This weird contains_key then unwrap get is to avoid unpleasant borrow checker errors
        if self.contains_key(&class_id) {
            return Ok(self.get(&class_id).unwrap().as_array());
        } else if class_files.get(&class_id).is_some() {
            return Ok(None);
        }

        // Otherwise, we load the class, if it has a classname.
        let (class_name, class_info) = class_names
            .name_from_gcid(class_id)
            .map_err(StepError::BadId)?;

        if !class_info.is_array() {
            // It isn't an array, but that's fine.
            return Ok(None);
        }

        let descriptor: DescriptorTypeCF<'static> = {
            // TODO: Return an error if this doesn't exist, but if it does not then that is sign
            // of an internal bug
            let (descriptor, remaining) = DescriptorTypeCF::parse(class_name.get())
                .map_err(StepError::DescriptorTypeError)?;
            // TODO: This should actually be a runtime error
            assert!(remaining.is_empty());
            // TODO: We shouldn't have to potentially allocate.
            descriptor.to_owned()
        };
        if let DescriptorTypeCF::Array { level, component } = descriptor {
            let component = DescriptorTypeBasic::from_class_file_desc(component, class_names);
            let id = self.load_level_array_of_desc_type_basic(
                class_names,
                class_files,
                packages,
                level,
                component,
            )?;
            debug_assert_eq!(id, class_id);

            // TODO: Better error handling than unwrap
            let array = self.get(&id).unwrap().as_array().unwrap();
            Ok(Some(array))
        } else {
            // TODO: This is likely indicative of an internal error since name parsing thought this was an array!
            Err(StepError::UnexpectedDescriptorType)
        }
    }

    /// Note: This specifically checks if it is a super class, if they are equal it returns false
    pub fn is_super_class(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_id: ClassId,
        maybe_super_class_id: ClassId,
    ) -> Result<bool, StepError> {
        let object_id = class_names.object_id();

        if class_id == maybe_super_class_id {
            return Ok(false);
        }

        class_files.load_by_class_path_id(class_names, class_id)?;

        // If this is an array, then it only extends the given class if it is java.lang.Object
        if let Some(_array_class) =
            self.get_array_class(class_names, class_files, packages, class_id)?
        {
            // Arrays only extend object
            return Ok(maybe_super_class_id == object_id);
        }

        // TODO: We could do a bit of optimization for if the class file was unloaded but the class
        // still existed
        // Load the class file, because we need the super id
        let mut current_class_id = class_id;
        loop {
            class_files.load_by_class_path_id(class_names, current_class_id)?;
            let class_file = class_files.get(&current_class_id).unwrap();

            if let Some(super_id) = class_file
                .get_super_class_id(class_names)
                .map_err(StepError::ClassFileIndex)?
            {
                if super_id == maybe_super_class_id {
                    return Ok(true);
                }

                current_class_id = super_id;
            } else {
                break;
            }
        }

        Ok(false)
    }

    pub fn implements_interface(
        &self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        class_id: ClassId,
        impl_interface_id: ClassId,
    ) -> Result<bool, StepError> {
        // Special handling for arrays
        if class_names.is_array(class_id).map_err(StepError::BadId)? {
            let interfaces = ArrayClass::get_interface_names();
            for interface_name in interfaces {
                let id = class_names.gcid_from_bytes(interface_name);
                if impl_interface_id == id {
                    return Ok(true);
                }
            }

            return Ok(false);
        }

        let mut current_class_id = Some(class_id);

        while let Some(current_id) = current_class_id {
            let interfaces = {
                class_files.load_by_class_path_id(class_names, current_id)?;
                let class_file = class_files.get(&current_id).unwrap();

                // Get all the interfaces. This is collected to a vec because we will invalidate the
                // class file reference
                let interfaces = class_file
                    .interfaces_indices_iter()
                    .collect::<SmallVec<[_; 8]>>();

                // Check all the topmost indices first
                for interface_index in interfaces.iter().copied() {
                    let interface_constant = class_file
                        .get_t(interface_index)
                        .ok_or(LoadClassError::BadInterfaceIndex(interface_index))?;
                    let interface_name =
                        class_file.get_text_b(interface_constant.name_index).ok_or(
                            LoadClassError::BadInterfaceNameIndex(interface_constant.name_index),
                        )?;
                    let interface_id = class_names.gcid_from_bytes(interface_name);

                    if interface_id == impl_interface_id {
                        return Ok(true);
                    }
                }

                interfaces
            };

            // Check if any of the interfaces implement it
            // This is done after the topmost interfaces are checked so that it makes those calls cheaper
            for interface_index in interfaces.iter().copied() {
                // Sadly, code can autocast an interface down to an interface that it extends
                // Ex: A extends B, B extends C
                // we can cast A down to C
                // The problem with this is that it requires loading every interface's class file..

                // We can't trust that the class file is still loaded.
                class_files.load_by_class_path_id(class_names, current_id)?;
                let class_file = class_files.get(&current_id).unwrap();

                let interface_constant = class_file
                    .get_t(interface_index)
                    .ok_or(LoadClassError::BadInterfaceIndex(interface_index))?;
                let interface_name = class_file.get_text_b(interface_constant.name_index).ok_or(
                    LoadClassError::BadInterfaceNameIndex(interface_constant.name_index),
                )?;
                let interface_id = class_names.gcid_from_bytes(interface_name);

                if self.implements_interface(
                    class_names,
                    class_files,
                    interface_id,
                    impl_interface_id,
                )? {
                    return Ok(true);
                }
            }

            class_files.load_by_class_path_id(class_names, current_id)?;
            let class_file = class_files.get(&current_id).unwrap();

            current_class_id = class_file
                .get_super_class_id(class_names)
                .map_err(StepError::ClassFileIndex)?;
        }

        Ok(false)
    }

    /// Checks if `class_id` is an array and can be downcasted to `target_id` (if it is an array)
    /// Ex: `java.lang.String[]` -> `Object[]`
    /// Note that this does not return true if they are of the same exact type
    /// That is because it is easy to determine from their class ids
    pub fn is_castable_array(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        class_id: ClassId,
        target_id: ClassId,
    ) -> Result<bool, StepError> {
        let class_array = if let Some(class_array) =
            self.get_array_class(class_names, class_files, packages, class_id)?
        {
            class_array
        } else {
            // It wasn't an array
            return Ok(false);
        };
        let class_elem = class_array.component_type();

        let target_array = if let Some(target_array) =
            self.get_array_class(class_names, class_files, packages, target_id)?
        {
            target_array
        } else {
            // It wasn't an array
            return Ok(false);
        };
        let target_elem = target_array.component_type();

        // If it isn't a class id then this would be comparison of primitive arrays which
        // can just be done by comparing the ids

        let class_elem_id = if let Some(class_elem_id) = class_elem.into_class_id() {
            class_elem_id
        } else {
            return Ok(false);
        };

        let target_elem_id = if let Some(target_elem_id) = target_elem.into_class_id() {
            target_elem_id
        } else {
            return Ok(false);
        };

        // if it can be cast down because it extends it (B[] -> A[])
        // if it can be cast down because target elem is an interface (A[] -> Cloneable[])
        // or if it can be cast down because it holds a castable array (B[][] -> A[][])
        Ok(self.is_super_class(
            class_names,
            class_files,
            packages,
            class_elem_id,
            target_elem_id,
        )? || self.implements_interface(
            class_names,
            class_files,
            class_elem_id,
            target_elem_id,
        )? || self.is_castable_array(
            class_names,
            class_files,
            packages,
            class_elem_id,
            target_elem_id,
        )?)
    }
}

// TODO: Will this behave incorrectly for classes which extend arrays? Those are incorrect, but
// should be properly handled.
// TODO: Should we rename these two iteration functions to something else to better represent
// that they include the base class?
/// Provides an 'iterator' over classes as it crawls up from the `class_id` given
/// Note that this *includes* the `class_id` given, and so you may want to skip over it.
#[must_use]
pub fn load_super_classes_iter(class_id: ClassId) -> SuperClassIterator {
    SuperClassIterator {
        scfi: SuperClassFileIterator::new(class_id),
    }
}

// TODO: It would be nice of SuperClassIterator could simply
// be implemented as a normal `.map` over super_class file iterator
// but SCFI borrows fields that this one needs and it wouldn't be able to access
// them.
pub struct SuperClassIterator {
    scfi: SuperClassFileIterator,
}
impl SuperClassIterator {
    pub fn next_item(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        classes: &mut Classes,
        packages: &mut Packages,
    ) -> Option<Result<ClassId, StepError>> {
        match self.scfi.next_item(class_names, class_files) {
            Some(Ok(id)) => Some(
                classes
                    .load_class(class_names, class_files, packages, id)
                    .map(|_| id),
            ),
            Some(Err(err)) => Some(Err(err)),
            None => None,
        }
    }
}

pub(crate) fn load_basic_descriptor_type(
    classes: &mut Classes,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    packages: &mut Packages,
    bdesc_type: DescriptorTypeBasic,
) -> Result<Option<ClassId>, StepError> {
    match bdesc_type {
        DescriptorTypeBasic::Class(class_id) => {
            classes.load_class(class_names, class_files, packages, class_id)?;
            Ok(Some(class_id))
        }
        _ => Ok(None),
    }
}

pub(crate) fn load_descriptor_type(
    classes: &mut Classes,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    packages: &mut Packages,
    desc_type: DescriptorType,
) -> Result<(), StepError> {
    match desc_type {
        DescriptorType::Basic(x) => {
            load_basic_descriptor_type(classes, class_names, class_files, packages, x)?;
            Ok(())
        }
        DescriptorType::Array { level, component } => {
            classes.load_level_array_of_desc_type_basic(
                class_names,
                class_files,
                packages,
                level,
                component,
            )?;

            Ok(())
        }
    }
}

/// Note: includes itself
pub fn does_extend_class(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &Classes,
    class_id: ClassId,
    desired_super_class_id: ClassId,
) -> Result<bool, StepError> {
    if class_id == desired_super_class_id {
        return Ok(true);
    }

    let super_class_id = if let Some(class) = classes.get(&class_id) {
        class.super_id()
    } else if let Some(class_file) = class_files.get(&class_id) {
        class_file
            .get_super_class_id(class_names)
            .map_err(StepError::ClassFileIndex)?
    } else {
        // The id should have already been registered by now
        class_files.load_by_class_path_id(class_names, class_id)?;
        let class_file = class_files
            .get(&class_id)
            .ok_or(StepError::MissingLoadedValue(
                "helper_does_extend_class : class_file",
            ))?;
        class_file
            .get_super_class_id(class_names)
            .map_err(StepError::ClassFileIndex)?
    };

    if let Some(super_class_id) = super_class_id {
        if super_class_id == desired_super_class_id {
            // It does extend it
            Ok(true)
        } else {
            // Crawl further up the tree to see if it extends it
            // Trees should be relatively small so doing recursion probably doesn't matter
            does_extend_class(
                class_names,
                class_files,
                classes,
                super_class_id,
                desired_super_class_id,
            )
        }
    } else {
        // There was no super class id so we're done here
        Ok(false)
    }
}
