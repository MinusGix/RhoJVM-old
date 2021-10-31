## rhojvm
The primary JVM implementation.

## rhojvm-base
Uses a fork of classfile-parser to load classfiles, load information about them, load methods, load and parse method instructions, and do verification.

## classfile-parser
Not currently in the repo.  
https://github.com/MinusGix/classfile-parser  
This is a fork that changes things to be harder to use incorrectly and provide various helper functions (such as type information on the indexes to make it easier to index into constant pool).