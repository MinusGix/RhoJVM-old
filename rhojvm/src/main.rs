use std::{num::NonZeroUsize, path::Path};

use rhojvm_base::{
    code::method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
    id::{ClassFileId, ClassId},
    Config, ProgramInfo, StepError,
};

fn main() {
    greedy_load();
    // prog.queue_load_class_by_class_file_id_cb(hwf_id, |prog, hw_id| {
    //     prog.queue_load_super_classes_cb(
    //         hw_id,
    //         |_, _| Ok(()),
    //         move |prog| {
    //             // Now that we've got the super tree, load the main
    //             let string_id = prog
    //                 .class_names
    //                 .gcid_from_slice(&["java", "lang", "String"]);
    //             prog.queue_load_method_by_desc_cb(
    //                 hw_id,
    //                 "main",
    //                 MethodDescriptor {
    //                     parameters: vec![DescriptorType::Array {
    //                         level: NonZeroUsize::new(1).unwrap(),
    //                         component: DescriptorTypeBasic::Class(string_id),
    //                     }],
    //                     return_type: None,
    //                 },
    //                 |prog, method_id| {
    //                     let method = prog.get_method(method_id).unwrap();
    //                     println!(
    //                         "Found main method! Access Flags: {:?}",
    //                         method.access_flags()
    //                     );
    //                     prog.queue_load_method_code(method_id, |prog, method_id, had_code| {});
    //                     Ok(())
    //                 },
    //             );
    //             prog.queue_for_all_methods(hw_id, |prog, method_id| {
    //                 prog.queue_init_method_overrides(method_id);
    //                 Ok(())
    //             });
    //             Ok(())
    //         },
    //     );
    //     Ok(())
    // });

    // prog.compute().unwrap();
}

fn noopr1<A>(_a: &mut A) -> Result<(), StepError>
where
    for<'a> A: 'a,
{
    Ok(())
}

fn noopr2<A, B>(_a: &mut A, _b: B) -> Result<(), StepError>
where
    for<'a> A: 'a,
{
    Ok(())
}

/// Tries loading as much types and information as it can
fn greedy_load() {
    let mut prog = ProgramInfo::new(Config {
        verify_method_access_flags: true,
    });
    let class_dirs = ["./ex/rt/", "./ex/jce/", "./ex/"];
    for path in class_dirs.into_iter() {
        let path = Path::new(path);
        prog.class_directories
            .add(path)
            .expect("for class directory to properly exist");
    }

    let hwf_id: ClassFileId = prog
        .queue
        .q_load_class_file_by_class_path_slice(
            &mut prog.class_names,
            &prog.class_files,
            &["HelloWorld"],
        )
        .unwrap();
    prog.queue
        .q_load_super_classes_cb(hwf_id, check_class, noopr1);

    prog.compute().unwrap()
}

fn check_class(prog: &mut ProgramInfo, class_id: ClassId) -> Result<(), StepError> {
    prog.queue.q_for_all_methods(class_id, |prog, method_id| {
        prog.queue.q_load_method_descriptor_types(method_id);
        prog.queue.q_verify_method_access_flags(method_id);
        prog.queue.q_verify_code_exceptions(method_id);
        // prog.queue.q_do_mut(move |prog| {
        //     let method = prog.methods.get(&method_id).unwrap();
        //     let desc_len = method.descriptor().parameters().len();
        //     for i in 0..desc_len {
        //         let method = prog.methods.get(&method_id).unwrap();
        //         let desc_type = &method.descriptor().parameters()[i];
        //         if let DescriptorType::Basic(DescriptorTypeBasic::Class(x)) = desc_type {
        //             let class_id = *x;
        //             check_class(prog, class_id)?;
        //         }
        //     }
        //     Ok(())
        // });
        Ok(())
    });
    Ok(())
}
