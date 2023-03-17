package rho;

// Note: Currently no stability guarantees are given for this class.
// Using this in an inappropriate manner can violate safety.
public class InternalField {
    // Internal classId for the class it is a part of
    public int classId;
    // Internal field index that it is within the given class referenced by classid
    public short fieldIndex;
    // The access flags / modifiers for the class field
    public short flags;

    // Private default constructor so it can't be called by outside callers
    // RhoJVM, however, does not use this and directly
    private InternalField () {}

    /// Returns the string representing the field's name
    public native String getName();
}