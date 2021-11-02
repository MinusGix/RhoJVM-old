# RhoJVM-base

This is the base library for taking in class-files, verifying them, getting type information, methods, instructions, and so on.  

The current structure of the library is based very much on having separate parts which store data and perhaps have minimal processing tied to them. This can be verbose, but the manual passing of fields to the methods allows avoiding the issue of the conglomerate structure (`ProgramInfo`) being borrowed in entirety when you only need a small subset.