package rho;

public final class SystemClassLoader extends ClassLoader {
    public static final SystemClassLoader systemLoader = SystemClassLoader.initializeSystemClassLoader();

    private static native SystemClassLoader initializeSystemClassLoader();
}