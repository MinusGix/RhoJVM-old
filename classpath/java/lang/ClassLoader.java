package java.lang;

import java.nio.ByteBuffer;
import java.util.Enumeration;
import java.net.URL;
import java.io.InputStream;
import java.io.IOException;
import java.security.ProtectionDomain;

import rho.SystemClassLoader;

public abstract class ClassLoader {
    private ClassLoader parent;

    protected ClassLoader(ClassLoader parent) {
        this.parent = parent;
    }

    protected ClassLoader() {
        this.parent = ClassLoader.getSystemClassLoader();
    }

    public Class<?> loadClass(String name) throws ClassNotFoundException {
        return SystemClassLoader.systemLoader.loadClass(name);
    }

    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        throw new UnsupportedOperationException("TODO: loadClass");
    }

    protected Object getClassLoadingLock(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    protected Class<?> findClass(String name) throws ClassNotFoundException {
        throw new ClassNotFoundException(name);
    }

    protected final Class<?> defineClass(byte[] data, int start, int length) throws ClassFormatError {
        throw new UnsupportedOperationException("TODO: defineClass");
    }

    protected final Class<?> defineClass(String name, byte[] data, int start, int length) throws ClassFormatError {
        throw new UnsupportedOperationException("TODO: defineClass 2");
    }

    protected final Class<?> defineClass(String name, byte[] data, int start, int length, ProtectionDomain protectionDomain) throws ClassFormatError {
        throw new UnsupportedOperationException("TODO: defineClass 3");
    }

    protected final Class<?> defineClass(String name, ByteBuffer data, ProtectionDomain protectionDomain) throws ClassFormatError {
        throw new UnsupportedOperationException("TODO: defineClass 4");
    }

    protected final void resolveClass(Class<?> clazz) {
        throw new UnsupportedOperationException("TODO: resolveClass");
    }

    protected final Class<?> findSystemClass(String name) throws ClassNotFoundException {
        throw new UnsupportedOperationException("TODO: findSystemClass");
    }

    protected final Class<?> findLoadedClass(String name) {
        throw new UnsupportedOperationException("TODO: findLoadedClass");
    }

    protected final void setSigners(Class<?> clazz, Object[] signers) {
        throw new UnsupportedOperationException("TODO: setSigners");
    }

    public URL getResource(String name) {
        return SystemClassLoader.systemLoader.getResource(name);
    }

    public InputStream getResourceAsStream(String name) {
        throw new UnsupportedOperationException("TODO: getResourceAsStream");
    }

    public Enumeration<URL> getResources(String name) throws IOException {
        throw new UnsupportedOperationException("TODO: getResources");
    }

    protected URL findResource(String name) {
        return null;
    }

    protected Enumeration<URL> findResources(String name) throws IOException {
        return java.util.Collections.emptyEnumeration();
    }

    protected static boolean registerAsParallelCapable() {
        throw new UnsupportedOperationException("TODO: registerAsParallelCapable");
    }

    public static URL getSystemResource(String name) {
        throw new UnsupportedOperationException("TODO: getSystemResource");
    }

    public static InputStream getSystemResourceAsStream(String name) {
        return SystemClassLoader.getSystemResourceAsStream(name);
    }

    public static Enumeration<URL> getSystemResources(String name) throws IOException {
        throw new UnsupportedOperationException("TODO: getSystemResources");
    }

    public final ClassLoader getParent() {
        return this.parent;
    }    

    public static ClassLoader getSystemClassLoader() {
        return SystemClassLoader.systemLoader;
    }

    protected Package definePackage(String name, String specificationTitle, String specificationVersion, String specificationVender, String implementationTitle, String implementationVersion, String implementationVendor, URL sealBase) {
        throw new UnsupportedOperationException("TODO: definePackage");
    }

    protected Package getPackage(String name) {
        throw new UnsupportedOperationException("TODO: getPackage");
    }

    protected Package[] getPackages() {
        throw new UnsupportedOperationException("TODO: getPackages");
    }

    protected String findLibrary(String libraryName) {
        throw new UnsupportedOperationException("TODO: findLibrary");
    }

    public void clearAssertionStatus() {
        throw new UnsupportedOperationException("TODO: clearAssertionStatus");
    }

    public void setDefaultAssertionStatus(boolean enabled) {
        throw new UnsupportedOperationException("TODO: setDefaultAssertionStatus");
    }

    public void setPackageAssertionStatus(String packageName, boolean enabled) {
        throw new UnsupportedOperationException("TODO: setPackageAssertionStatus");
    }

    public void setClassAssertionStatus(String className, boolean enabled) {
        throw new UnsupportedOperationException("TODO: setClassAssertionStatus");
    }
}