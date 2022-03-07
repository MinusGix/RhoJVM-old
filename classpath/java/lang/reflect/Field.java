package java.lang.reflect;

import rho.InternalField;
import java.lang.annotation.Annotation;
import java.lang.reflect.AnnotatedType;
import java.lang.reflect.Type;

public final class Field extends AccessibleObject implements Member {
    // For checking if the field is synthetic
    private static final int ACC_SYNTHETIC = 0x1000;

    private final InternalField internalField;

    // For use by rhojvm
    private Field(InternalField internalField) {
        this.internalField = internalField;
    }

    public Class<?> getDeclaringClass() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public String getName() {
        return internalField.getName();
    }

    public int getModifiers() {
        return internalField.flags;
    }

    public boolean isEnumConstant() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isSynthetic() {
        return (internalField.flags & ACC_SYNTHETIC) != 0;
    }

    public Class<?> getType() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Type getGenericType() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }
    public boolean equals(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public int hashcode() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public String toString() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public String toGenericString() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Object get(Object obj) throws IllegalArgumentException, IllegalAccessException {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public AnnotatedType getAnnotatedType() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public<T extends Annotation> T getAnnotation(Class<T> annotationClass) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public<T extends Annotation> T[] getAnnotationsByType(Class<T> annotationClass) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Annotation[] getDeclaredAnnotations() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public double getDouble(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public float getFloat(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean getBoolean(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public byte getByte(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public char getChar(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public short getShort(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public int getInt(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public long getLong(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void set(Object obj, Object value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void setDouble(Object obj, double d) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void setFloat(Object obj, float f) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void setBoolean(Object obj, boolean z) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void setByte(Object obj, byte b) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void setChar(Object obj, char c) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void setShort(Object obj, short s) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void setInt(Object obj, int i) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void setLong(Object obj, long l) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }
}