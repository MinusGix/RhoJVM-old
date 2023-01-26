package java.lang.reflect;

import java.lang.annotation.Annotation;
import java.lang.reflect.AnnotatedType;
import java.lang.reflect.Type;
import java.lang.reflect.AccessibleObject;
import java.lang.reflect.GenericDeclaration;

public abstract class Executable extends AccessibleObject implements Member, GenericDeclaration {
    public AnnotatedType[] getAnnotatedExceptionTypes() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public AnnotatedType[] getAnnotatedParameterTypes() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public AnnotatedType getAnnotatedReceiverType() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public abstract AnnotatedType getAnnotatedReturnType();

    public <T extends Annotation> T getAnnotation(Class<T> annClass) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public <T extends Annotation> T[] getAnnotationsByType(Class<T> annClass) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Annotation[] getDeclaredAnnotations() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public abstract Class<?> getDeclaringClass();

    public abstract Class<?>[] getExceptionTypes();

    public Type[] getGenericExceptionTypes() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Type[] getGenericParameterTypes() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public abstract int getModifiers();
    public abstract String getName();

    public abstract Annotation[][] getParameterAnnotations();

    public int getParameterCount() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Parameter[] getParameters() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public abstract Class<?>[] getParameterTypes();

    public abstract TypeVariable<?>[] getTypeParameters();

    public boolean isSynthetic() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isVarArgs() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public abstract String toGenericString();
}