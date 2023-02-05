package rho;

import java.io.InputStream;
import java.util.Enumeration;
import java.util.Vector;
import java.net.URL;
import java.io.IOException;

public final class SystemClassLoader extends ClassLoader {
    public static final SystemClassLoader systemLoader = SystemClassLoader.initializeSystemClassLoader();

    private static native SystemClassLoader initializeSystemClassLoader();

    public native Class<?> loadClass(String name) throws ClassNotFoundException;

    public native static InputStream getSystemResourceAsStream(String name);

    public native URL getResource(String name);

    public native Enumeration<URL> getResources(String name) throws IOException;

    public native InputStream getResourceAsStream(String name);
}