package java.lang;

import java.io.PrintStream;
import java.io.InputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.util.Map;
import java.util.Properties;
import java.lang.SecurityManager;

public final class System {
    // Prevent it from being created
    private System() {}

    // TODO: Buffer these?
    public static final PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true);
    public static final PrintStream err = new PrintStream(new FileOutputStream(FileDescriptor.err), true);
    public static final PrintStream in = new PrintStream(new FileOutputStream(FileDescriptor.in), true);

    private static SecurityManager securityManager = null;

    public static void arraycopy(Object src, int srcOffset, Object dest, int destOffset, int length) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static String getProperty(String name) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static String getProperty(String name, String defaultValue) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static String setProperty(String name, String value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static Properties getProperties() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static long currentTimeMillis() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static int identityHashCode(Object o) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static long nanoTime() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static String mapLibraryName(String name) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static void load(String path) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static void loadLibrary(String name) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static void gc() {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static void exit(int code) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static SecurityManager getSecurityManager() {
        return securityManager;
    }

    public static void setSecurityManager(SecurityManager securityManager) {
        System.securityManager = securityManager;
    }

    public static String getenv(String name) throws NullPointerException {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static Map<String, String> getenv() throws SecurityException {
        throw new UnsupportedOperationException("TODO: Implement this");
    }
}