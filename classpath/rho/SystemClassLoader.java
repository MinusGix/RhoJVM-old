package rho;

import java.io.InputStream;

public final class SystemClassLoader extends ClassLoader {
    public static final SystemClassLoader systemLoader = SystemClassLoader.initializeSystemClassLoader();

    private static native SystemClassLoader initializeSystemClassLoader();

    public native static InputStream getSystemResourceAsStream(String name);
}