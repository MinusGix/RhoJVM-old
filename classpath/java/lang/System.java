package java.lang;

import java.io.BufferedInputStream;
import java.io.PrintStream;
import java.io.InputStream;
import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.util.Map;
import java.util.Properties;
import java.lang.SecurityManager;
import java.util.Properties;

import rho.util.Log;

public final class System {
    private static Properties props = new Properties();
    static {
        System.setProperties(props);
    }
    
    private static native void setProperties(Properties props);

    // Prevent it from being created
    private System() {}

    // TODO: Buffer these?
    // TODO: These are defined as final but setErr and friends can modify them?? Should we have a special marker that says that the field is final but it can be modified? Or should we just make a class that extends PrintStream and let you override its internal true printstream?
    public static /* final */ PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true);
    public static /* final */ PrintStream err = new PrintStream(new FileOutputStream(FileDescriptor.err), true);
    public static /* final */ InputStream in = new BufferedInputStream(new FileInputStream(FileDescriptor.in));

    private static SecurityManager securityManager = null;

    public static void setIn(InputStream in) {
        System.in = in;
    }

    public static void setOut(PrintStream out) {
        System.out = out;
    }

    public static void setErr(PrintStream err) {
        System.err = err;
    }

    public static native void arraycopy(Object src, int srcOffset, Object dest, int destOffset, int length);

    public static String getProperty(String name) {
        // TODO: Security checks
        if (name == null) {
            throw new NullPointerException("name");
        }

        if (name.isEmpty()) {
            throw new IllegalArgumentException("Empty property name");
        }

        Log.info("getProperty(" + name + ") = " + System.props.getProperty(name));
        return System.props.getProperty(name);
    }

    public static String getProperty(String name, String defaultValue) {
        // TODO: Security checks
        if (name == null) {
            throw new NullPointerException("name");
        }

        if (name.isEmpty()) {
            throw new IllegalArgumentException("Empty property name");
        }

        Log.info("getProperty(" + name + ", " + defaultValue + ") = " + System.props.getProperty(name, defaultValue));
        return System.props.getProperty(name, defaultValue);
    }

    public static String setProperty(String name, String value) {
        Log.info("setProperty(" + name + ", " + value + ")");
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
        Log.info("clearProperty(" + name + ")");
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

    public static native long currentTimeMillis();

    public static int identityHashCode(Object o) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static native long nanoTime();

    public native static String mapLibraryName(String name);

    public static native void load(String path);

    public static native void loadLibrary(String name);

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