package java.lang.invoke;

import java.lang.reflect.Method;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;

import rho.invoke.MethodHandleInfoInst;

public class MethodHandles {
    public static final class Lookup {
        public static final int PUBLIC = 1;
        public static final int PRIVATE = 2;
        public static final int PROTECTED = 4;
        public static final int PACKAGE = 8;

        // TODO: Remove
        static final Lookup IMPL_LOOKUP = new Lookup(void.class);

        private Class<?> referent = null;

        Lookup (Class<?> referent) {
            this.referent = referent;
        }

        public MethodHandle findConstructor(Class<?> target, MethodType type) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle findGetter(Class<?> target, String name, Class<?> type) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle findSetter(Class<?> target, String name, Class<?> type) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle findSpecial(Class<?> target, String name, MethodType type, Class<?> specialCaller) {
            throw new UnsupportedOperationException();
        }

        public native MethodHandle findStatic(Class<?> target, String name, MethodType type);

        public MethodHandle findStaticGetter(Class<?> target, String name, Class<?> type) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle findStaticSetter(Class<?> target, String name, Class<?> type) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle findVirtual(Class<?> target, String name, MethodType type) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle bind(Object recv, String name, MethodType typ) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle unreflect(Method method) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle unreflectConstructor(Constructor<?> constr) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle unreflectGetter(Field field) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle unreflectSetter(Field field) {
            throw new UnsupportedOperationException();
        }

        public MethodHandle unreflectSpecial(Method method, Class<?> caller) {
            throw new UnsupportedOperationException();
        }

        public native Class<?> lookupClass();

        public Lookup in(Class<?> target) {
            throw new UnsupportedOperationException();
        }

        public int lookupModes() {
            // TODO: This can be restricted
            return PUBLIC | PRIVATE | PROTECTED | PACKAGE;
        }

        public MethodHandleInfo revealDirect(MethodHandle target) {
            return MethodHandles.revealDirect(target);
        }

        public String toString() {
            throw new UnsupportedOperationException();
        }
    }

    // private static Lookup lookupInst = new Lookup();

    public native static Lookup lookup();
    
    private static native MethodHandleInfo revealDirect(MethodHandle target);

    public static native MethodHandle constant(Class<?> type, Object value);
}