use rhojvm_base::data::{class_files::ClassFiles, class_names::ClassNames};

use crate::{
    class_instance::{ClassInstance, PrimitiveArrayInstance},
    eval::EvalError,
    gc::{Gc, GcRef},
    rv::RuntimeValuePrimitive,
    GeneralError, State,
};

// TODO: The GC needs to be pay attention to this!
#[derive(Default, Debug, Clone)]
pub struct StringInterner {
    /// Vector of (String, char[])
    /// The char[] is the internal array for the string
    /// This is to speed up lookup
    data: Vec<(GcRef<ClassInstance>, GcRef<PrimitiveArrayInstance>)>,
}
impl StringInterner {
    pub fn get_by_data(
        &self,
        gc: &Gc,
        value: &[RuntimeValuePrimitive],
    ) -> Option<GcRef<ClassInstance>> {
        self.data
            .iter()
            .find(|(_, data)| {
                let data = gc.deref(*data).unwrap();
                data.elements == value
            })
            .map(|(re, _)| *re)
    }

    pub fn has_by_data(&self, gc: &Gc, value: &[RuntimeValuePrimitive]) -> bool {
        self.get_by_data(gc, value).is_some()
    }

    /// Note: java/lang/String should already be loaded
    pub fn intern(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &ClassFiles,
        state: &mut State,
        target_ref: GcRef<ClassInstance>,
    ) -> Result<GcRef<ClassInstance>, GeneralError> {
        // If it doesn't exist then return None
        let _ = state
            .gc
            .deref(target_ref)
            .ok_or(EvalError::InvalidGcRef(target_ref.into_generic()));

        let string_id = class_names.gcid_from_bytes(b"java/lang/String");
        let string_data_field_id = state.get_string_data_field(class_files, string_id)?;

        let target_data_ref = state
            .gc
            .deref(target_ref)
            .ok_or(EvalError::InvalidGcRef(target_ref.into_generic()))?
            .fields
            .get(string_data_field_id)
            .ok_or(EvalError::MissingField(string_data_field_id))?
            .value();
        let target_data_ref = target_data_ref.into_reference().unwrap();
        let target_data_ref = target_data_ref.expect("TODO: NPE?");
        let target_data_ref: GcRef<PrimitiveArrayInstance> = target_data_ref.unchecked_as();

        // TODO: Debug assert that the char[] field is paired with the right string
        for (interned_string_ref, interned_data_ref) in self.data.iter().copied() {
            if interned_string_ref == target_ref {
                // If they're the same reference, then great, no need to do anything
                return Ok(target_ref);
            }

            let interned_data = state.gc.deref(interned_data_ref).unwrap();
            let target_data = state.gc.deref(target_data_ref).unwrap();

            debug_assert_eq!(interned_data.element_type, target_data.element_type);

            if interned_data.elements == target_data.elements {
                return Ok(interned_string_ref);
            }
        }

        // Otherwise, it weasn't in the interner
        self.data.push((target_ref, target_data_ref));

        Ok(target_ref)
    }
}
