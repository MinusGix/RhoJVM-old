# RhoJVM
This is an implementation of the JVM based on the rhojvm-base library.  
It is very much in development, and so should not be relied upon for speed, accuracy, safety, or validity.  
A sub-goal of this is to allow a JVM which is hopefully manipulable from Rust, to allow tight interaction between them.  
Another sub-goal is to allow the generation of class-files, but that is currently low on the target due to the complexity inherent in constructing and validating them. 