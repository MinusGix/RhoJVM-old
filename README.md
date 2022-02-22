# RhoJVM
This is an implementation of the JVM based on the rhojvm-base library. Note that this is currently **not** a compliant JVM implementation.  
It is very much in development, and so should not be relied upon for speed, accuracy, safety, or validity.  
A sub-goal of this is to allow a JVM which is hopefully manipulable from Rust, to allow tight interaction between them.  
Another sub-goal is to allow the generation of class-files, but that is currently low on the target due to the complexity inherent in constructing and validating them. 

## Safety
The basic JVM parts for interpreting are actually rather safe, since I did not need to use unsafe code to write them. Though, they depend on libraries which use `unsafe`.  
The next is that currently this JVM implementation does not do any multithreading properly. It may spontaneously combust before you get there, but it currently does not do any of the locking needed for that.  
A notable, but unfixable, issue with safety is that the JVM has native methods which are arbitrary methods defined with the C calling convention, and so can do arbitrarily unsafe things.

## License
Currently I am going with this being under MIT/Apache if possible, like other Rust projects. However, this should _not_ be relied upon at this moment since I am still quite unsure of the exact licensing scenario.  
- This code relies on the official [JVM specifications](https://docs.oracle.com/javase/specs/jvms/se8/html/index.html) (and several of the future versions) as well as the [JNI specification](https://docs.oracle.com/en/java/javase/17/docs/specs/jni/index.html). Is there any extra licensing bits to be aware of there?  
- Deliberately avoided looking at the OpenJDK implementation, which unless the above implies a different license, means that we don't have to be GPL.