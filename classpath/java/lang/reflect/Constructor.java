package java.lang.reflect;

import java.lang.annotation.Annotation;
import java.lang.reflect.AnnotatedType;
import java.lang.reflect.Type;

public final class Constructor<T> extends Executable {
    private Class<T> clazz;
    private short method_index;

    Constructor(Class<T> clazz, short method_index) {
        this.clazz = clazz;
        this.method_index = method_index;
    }

    public T newInstance(Object... args) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    @Override
    public Class<T> getDeclaringClass() {
        return this.clazz;
    }

    @Override
    public String getName() {
        return this.clazz.getName();
    }

    @Override
    public int getModifiers() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    // === Parameters ===

    @Override
    public TypeVariable<Constructor<T>>[] getTypeParameters() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    @Override
    public Class<?>[] getParameterTypes() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public int getParameterCount() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    @Override
    public Type[] getGenericParameterTypes() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    // === Exceptions ===

    @Override
    public Class<?>[] getExceptionTypes() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    @Override
    public Type[] getGenericExceptionTypes() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    // === Annotations ===
    @Override
    public Annotation[][] getParameterAnnotations() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    @Override
    public AnnotatedType getAnnotatedReturnType() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    // === Other ===

    // TODO: smarter equals
    // TODO: smarter hashcode
    // TODO: toString

    @Override
    public String toGenericString() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }
}