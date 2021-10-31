# RhoJVM-base

This is the base library for taking in class-files, verifying them, getting type information, methods, instructions, and so on.  
  
The current way that the library works is primarily through a Command-based structure with callbacks to allow partial loading and verification of data, with helper functions to make more normal function calling available.  
  
This tries to allow invalid (according to verification rules) JVM code, but provides verification Commands that should be used by a library that cares. Some code may want to accept code that breaks those rules (such as a decompiler), and so there aims to be support, but it does try to push towards verification.  
  
An important part of the codebase is to have so order of loading classes does not matter and to allow serialization of the information. The `ClassFileId`/`ClassId`/`MethodId`/`PackageId` are based on the access path (ex: `java/lang/Object`), which makes computing them without loading the class easier, and so that they do not depend on load-order (like an incremental id would).  
A pain point with this is that the Command-based structure is very dependent on callbacks, which obviously can't be serialized. Most likely, it will be allowed to simply drop all commands when it is put to file. Note that this does not mean that commands can be arbitrarily dropped, but only if serialization is performed.