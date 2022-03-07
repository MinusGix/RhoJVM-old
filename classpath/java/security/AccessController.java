package java.security;

public final class AccessController {
    public static void checkPermission(Permission perm) throws AccessControlException {
        // FIXME: Check permission.
    }

    public static<T> T doPrivileged(PrivilegedAction<T> action) {
        return action.run();
    }

    public static<T> T doPrivileged(PrivilegedAction<T> action, AccessControlContext context) {
        // TODO: Restrict privileges with the context
        return action.run();
    }

    public static<T> T doPrivileged(PrivilegedAction<T> action, AccessControlContext context, Permission... perms) {
        // TODO: Restrict the privileges with the context and perms
        return action.run();
    }

    public static<T> T doPrivileged(PrivilegedExceptionAction<T> action, AccessControlContext context) throws Exception {
        // TODO: Restrict privileges with the context
        return action.run();
    }

    public static<T> T doPrivileged(PrivilegedExceptionAction<T> action, AccessControlContext context, Permission... perms) throws Exception {
        // TODO: Restrict privileges with the context and perms
        return action.run();
    }

    public static<T> T doPrivilegedWithCombiner(PrivilegedAction<T> action) {
        return action.run();
    }

    public static<T> T doPrivilegedWithCombiner(PrivilegedAction<T> action, AccessControlContext context, Permission... perms) {
        // TODO: Restrict privileges with the context and perms
        return action.run();
    }

    public static<T> T doPrivilegedWithCombiner(PrivilegedExceptionAction<T> action) throws Exception {
        return action.run();
    }

    public static<T> T doPrivilegedWithCombiner(PrivilegedExceptionAction<T> action, AccessControlContext context, Permission... perms) throws Exception {
        // TODO: Restrict privileges with the context and perms
        return action.run();
    }

    public static AccessControlContext getContext() {
        throw new UnsupportedOperationException("TODO: Implement getContext");
    }
}