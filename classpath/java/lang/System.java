package java.lang;

import java.io.PrintStream;
import java.io.InputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.util.Map;
import java.util.Properties;
import java.lang.SecurityManager;
import java.util.Properties;

public final class System {
    private static Properties props = new Properties();
    static {
        // TODO: Platform-specific
        System.props.setProperty("file.separator", "/");
        // TODO: Platform-specific
        System.props.setProperty("line.separator", "\n");
        
        System.props.setProperty("file.encoding", "UTF-8");
    }

    // Prevent it from being created
    private System() {}

    // TODO: Buffer these?
    public static final PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true);
    public static final PrintStream err = new PrintStream(new FileOutputStream(FileDescriptor.err), true);
    public static final PrintStream in = new PrintStream(new FileOutputStream(FileDescriptor.in), true);

    private static SecurityManager securityManager = null;

    public static native void arraycopy(Object src, int srcOffset, Object dest, int destOffset, int length);

    public static String getProperty(String name) {
        // TODO: Security checks
        if (name == null) {
            throw new NullPointerException("name");
        }

        if (name.isEmpty()) {
            throw new IllegalArgumentException("Empty property name");
        }
        return System.props.getProperty(name);
    }

    public static String getProperty(String name, String defaultValue) {
        // TODO: Security checks
        if (name == null) {
            throw new NullPointerException("name");
        } else if (defaultValue == null) {
            throw new NullPointerException(defaultValue);
        }

        if (name.isEmpty()) {
            throw new IllegalArgumentException("Empty property name");
        }
        return System.props.getProperty(name, defaultValue);
    }

    public static String setProperty(String name, String value) {
        // TODO: Security checks
        if (name == null) {
            throw new NullPointerException("name");
        } else if (value == null) {
            throw new NullPointerException(value);
        }

        if (name.isEmpty()) {
            throw new IllegalArgumentException("Empty property name");
        }
        return (String) System.props.setProperty(name, value);
    }

    public static String clearProperty(String name) {
        if (name == null) {
            throw new NullPointerException("name");
        }

        if (name.isEmpty()) {
            throw new IllegalArgumentException("Empty property name");
        }

        return (String) System.props.remove(name);
    }

    public static Properties getProperties() {
        return System.props;
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