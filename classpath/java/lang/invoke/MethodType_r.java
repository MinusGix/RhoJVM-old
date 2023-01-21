package java.lang.invoke;

import java.util.List;

// NOTE: This class is assumed to have zero fields by the jvm to make it cheaper/simpler to
// construct!
public final class MethodType {
    // === Constructors ===

    public static MethodType methodType(Class<?> returnTy) {
        throw new UnsupportedOperationException();
    }

    public static MethodType methodType(Class<?> returnTy, Class<?> paramTy) {
        throw new UnsupportedOperationException();
    }

    public static MethodType methodType(Class<?> returnTy, Class<?>[] paramsTy) {
        throw new UnsupportedOperationException();
    }

    public static MethodType methodType(Class<?> returnTy, List<Class<?>> paramsTy) {
        throw new UnsupportedOperationException();
    }

    public static MethodType methodType(Class<?> returnTy, Class<?> paramTy, Class<?>... paramsTy) {
        throw new UnsupportedOperationException();
    }

    public static MethodType methodType(Class<?> returnTy, MethodType ty) {
        throw new UnsupportedOperationException();
    }

    public static MethodType fromMethodDescriptorString(String desc, ClassLoader loader) {
        throw new UnsupportedOperationException();
    }

    public static MethodType genericMethodType(int count) {
        throw new UnsupportedOperationException();
    }

    public static MethodType genericMethodType(int count, boolean includeArraySuffix) {
        throw new UnsupportedOperationException();
    }

    // === Parameter Modification ===

    public MethodType appendParameterTypes(Class<?>... paramTy) {
        throw new UnsupportedOperationException();
    }

    public MethodType appendParameterTypes(List<Class<?>> paramTy) {
        throw new UnsupportedOperationException();
    }

    public MethodType changeParameterType(int idx, Class<?> ty) {
        throw new UnsupportedOperationException();
    }

    public MethodType dropParameterTypes(int start, int end) {
        throw new UnsupportedOperationException();
    }

    public MethodType insertParameterTypes(int idx, Class<?>... paramTy) {
        throw new UnsupportedOperationException();
    }

    public MethodType insertParameterTypes(int idx, List<Class<?>> paramTy) {
        throw new UnsupportedOperationException();
    }

    // === Return Modification ===

    public MethodType changeReturnType(Class<?> ty) {
        throw new UnsupportedOperationException();
    }

    // === General Modification ===

    public MethodType erase() {
        throw new UnsupportedOperationException();
    }

    public MethodType generic() {
        throw new UnsupportedOperationException();
    }

    public MethodType unwrap() {
        throw new UnsupportedOperationException();
    }

    public MethodType wrap() {
        throw new UnsupportedOperationException();
    }

    // === Information ===

    public boolean hasPrimitives() {
        throw new UnsupportedOperationException();
    }

    public boolean hasWrappers() {
        throw new UnsupportedOperationException();
    }

    // === Parameter Information ===

    public Class<?>[] parameterArray() {
        throw new UnsupportedOperationException();
    }

    public int parameterCount() {
        throw new UnsupportedOperationException();
    }

    public Class<?> parameterType(int idx) {
        throw new UnsupportedOperationException();
    }

    // === Return Information ===

    public Class<?> returnType() {
        throw new UnsupportedOperationException();
    }

    // === Conversion ===

    public String toMethodDescriptorString() {
        throw new UnsupportedOperationException();
    }

    public String toString() {
        throw new UnsupportedOperationException();
    }

    // === Other ===

    public boolean equals(Object obj) {
        throw new UnsupportedOperationException();
    }

    public int hashCode() {
        throw new UnsupportedOperationException();
    }
}