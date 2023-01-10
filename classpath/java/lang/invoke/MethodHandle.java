package java.lang.invoke;

import java.util.List;

// Method handle is annoying.
// It isn't *just* a handle to some unknown method, but rather it is a handle to a variety
// of potential operations, each of which behaves very differently. They also tend to emulate 
// instructions.
// Such as there being invokestatic/invokespecial/getfield/putfield/getfieldstatic/etc.
// This makes it more annoying to implement.

// We implement it with a rho/invoke/MethodHandle (since this is abstract) which just has an enum
// of the various types in the internal jvm implementation

// See the official java documentation for methodhandle for various complexities that this has.

// NOTE: This class is assumed to have zero fields by the jvm to make it cheaper/simpler to 
// construct!
public abstract class MethodHandle {
    public MethodHandle asCollector(Class<?> arrayType, int length) {
        throw new UnsupportedOperationException();
    }

    public MethodHandle asFixedArity() {
        throw new UnsupportedOperationException();
    }

    public MethodHandle asSpread(Class<?> arrayType, int length) {
        throw new UnsupportedOperationException();
    }

    public MethodHandle asVarargsCollector(Class<?> arrayType) {
        throw new UnsupportedOperationException();
    }

    public MethodHandle bindTo(Object target) {
        throw new UnsupportedOperationException();
    }

    public Object invoke(Object... args) {
        throw new UnsupportedOperationException();
    }

    public Object invokeWithArguments(List<?> arguments) {
        throw new UnsupportedOperationException();
    }

    public Object invokeWithArguments(Object... arguments) {
        throw new UnsupportedOperationException();
    }

    public boolean isVarargsCollector() {
        throw new UnsupportedOperationException();
    }

    public MethodType type() {
        throw new UnsupportedOperationException();
    }

    public String toString() {
        throw new UnsupportedOperationException();
    }
}