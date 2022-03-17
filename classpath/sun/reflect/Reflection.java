package sun.reflect;

import java.lang.reflect.Method;

public class Reflection {
    public static native Class<?> getCallerClass();

    public static int getClassAccessFlags(Class<?> clazz) {
        throw new UnsupportedOperationException();
    }

    public static boolean quickCheckMemberAccess(Class<?> clazz, int modifiers) {
        throw new UnsupportedOperationException();
    }

    public static void ensureMemberAccess(Class<?> current, Class<?> member, Object target, int modifiers) throws IllegalAccessException {
        if (current == null || member == null) {
            throw new InternalError();
        }

        if(!Reflection.verifyMemberAccess(current, member, target, modifiers)) {
            throw new IllegalAccessException();
        }
    }

    public static boolean verifyMemberAccess(Class<?> current, Class<?> member, Object target, int modifiers) {
        // FIXME: Don't silently allow access
        return true;
    }

    public static boolean isSameClassPackage(Class<?> left, Class<?> right) {
        throw new UnsupportedOperationException();
    }

    public static boolean isCallerSensitive(Method method) {
        throw new UnsupportedOperationException();
    }
}