package java.lang;

import java.lang.reflect.AnnotatedElement;
import java.lang.reflect.Constructor;
import java.lang.reflect.GenericDeclaration;
import java.lang.reflect.Method;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.Proxy;
import java.lang.reflect.Type;
import java.lang.reflect.TypeVariable;
import java.lang.annotation.Annotation;
import java.io.InputStream;
import java.net.URL;
import java.security.ProtectionDomain;

import rho.SystemClassLoader;

public final class Class<T> implements AnnotatedElement, GenericDeclaration, Type {
    // TODO: We could make this final
    // internal class id
    private int classId;

    // So that it can't be constructed manually
    // Note that this is not actually called, the fields are filled in directly.
    private Class() {}

    // Get the class for a primitive class-type, such as Float
    static native Class getPrimitiveClass(String name);

    public static Class forName(String name) throws ClassNotFoundException {
        return Class.getClassForName(name);
    }

    public static Class forName(String name, boolean initialize, ClassLoader loader) throws ClassNotFoundException {
        return Class.getClassForNameWithClassLoader(name, initialize, loader);
    }

    // We have these as separate functions because looking up the nonoverloaded versions requires
    // less processing/generation of text
    private static native Class getClassForName(String name);
    private static native Class getClassForNameWithClassLoader(String name, boolean initialize, ClassLoader loader) throws ClassNotFoundException;

    // public static Class forName(Module module, String name {
    //     throw new UnsupportedOperationException("TODO: Implement this");
    // }

    public native String getName();

    public String getCanonicalName() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public String getSimpleName() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public<U> Class<? extends U> asSubClass(Class<U> clazz) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public T cast(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean desiredAssertionStatus() {
        return false;
    }

    // public AnnotatedType[] getAnnotatedInterfaces() {
    //     throw new UnsupportedOperationException("TODO: Implement this");
    // }

    // public AnnotatedType getAnnotatedSuperClass() {
    //     throw new UnsupportedOperationException("TODO: Implement this");
    // }

    public<A extends Annotation> A[] getAnnotationsByType(Class<A> annotationClass) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Class<?>[] getClasses() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public ClassLoader getClassLoader() {
        // FIXME: This won't always be correct
        return SystemClassLoader.systemLoader;
    }

    public native Class<?> getComponentType();

    public Constructor<T> getConstructor(Class<?>... parameterTypes) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Constructor<?>[] getConstructors() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public<A extends Annotation> A getDeclaredAnnotation(Class<A> annotationClass) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public<A extends Annotation> A[] getDeclaredAnnotationsByType(Class<A> annotationClass) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Class<?>[] getDeclaredClasses() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Constructor<T> getDeclaredConstructor(Class<?>... parameterTypes) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Constructor<?>[] getDeclaredConstructors() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    // Defined by rhojvm
    public native Field getDeclaredField(String name);

    public Field[] getDeclaredFields() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Method getDeclaredMethod(String name, Class<?>... parameterTypes) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Method[] getDeclaredMethods() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Class<?> getDeclaringClass() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Class<?> getEnclosingClass() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Constructor<?> getEnclosingConstructor() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Method getEnclosingMethod() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public T[] getEnumConstants() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Field getField(String name) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Field[] getFields() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Type[] getGenericInterfaces() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Type getGenericSuperClass() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Class<?>[] getInterfaces() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Method getMethod(String name, Class<?>... parameterTypes) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Method[] getMethods() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public int getModifiers() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    // public Module getModule() {
    //     throw new UnsupportedOperationException("TODO: Implement this");
    // }

    public Class<?> getNestHost() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Class<?>[] getNestMembers() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public native Package getPackage();

    public String getPackageName() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public ProtectionDomain getProtectionDomain() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public URL getResource(String name) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public InputStream getResourceAsStream(String name) {
        // FIXME: Get class loader
        return ClassLoader.getSystemResourceAsStream(name);
    }

    public Object[] getSigners() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Class<? super T> getSuperclass() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public String getTypeName() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isAnnotation() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isAnnotationPresent(Class<? extends Annotation> annotationClass) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isAnonymousClass() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public native boolean isArray();

    public boolean isAssignableFrom(Class<?> cls) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isEnum() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isInstance(Object obj) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isInterface() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isLocalClass() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isMemberClass() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean isNestmateOf(Class<?> c) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public native boolean isPrimitive();

    public boolean isSynthetic() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public native T newInstance() throws IllegalAccessException, InstantiationException;

    public String toGenericString() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public String toString() {
        String output;
        if (isInterface()) {
            output = "interface ";
        } else if (isAnnotation()) {
            output = "annotation";
        } else {
            output = "class";
        }

        output += getName();

        return output;
    }

    // === AnnotatedElement functions ===
    @Override
    public Annotation[] getDeclaredAnnotations() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    @Override
    public Annotation[] getAnnotations() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    @Override
    public <T extends Annotation> T getAnnotation(Class<T> class_) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    // === GenericDeclaration functions ===
    @Override
    public TypeVariable<?>[] getTypeParameters() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }
}