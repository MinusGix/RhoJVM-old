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
        throw new UnsupportedOperationException("TODO");
    }

    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        throw new UnsupportedOperationException("TODO");
    }

    protected Object getClassLoadingLock(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    protected Class<?> findClass(String name) throws ClassNotFoundException {
        throw new ClassNotFoundException(name);
    }

    protected final Class<?> defineClass(byte[] data, int start, int length) throws ClassFormatError {
        throw new UnsupportedOperationException("TODO");
    }

    protected final Class<?> defineClass(String name, byte[] data, int start, int length) throws ClassFormatError {
        throw new UnsupportedOperationException("TODO");
    }

    protected final Class<?> defineClass(String name, byte[] data, int start, int length, ProtectionDomain protectionDomain) throws ClassFormatError {
        throw new UnsupportedOperationException("TODO");
    }

    protected final Class<?> defineClass(String name, ByteBuffer data, ProtectionDomain protectionDomain) throws ClassFormatError {
        throw new UnsupportedOperationException("TODO");
    }

    protected final void resolveClass(Class<?> clazz) {
        throw new UnsupportedOperationException("TODO");
    }

    protected final Class<?> findSystemClass(String name) throws ClassNotFoundException {
        throw new UnsupportedOperationException("TODO");
    }

    protected final Class<?> findLoadedClass(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    protected final void setSigners(Class<?> clazz, Object[] signers) {
        throw new UnsupportedOperationException("TODO");
    }

    public URL getResource(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    public InputStream getResourceAsStream(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    public Enumeration<URL> getResources(String name) throws IOException {
        throw new UnsupportedOperationException("TODO");
    }

    protected URL findResource(String name) {
        return null;
    }

    protected Enumeration<URL> findResources(String name) throws IOException {
        return java.util.Collections.emptyEnumeration();
    }

    protected static boolean registerAsParallelCapable() {
        throw new UnsupportedOperationException("TODO");
    }

    public static URL getSystemResource(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    public static InputStream getSystemResourceAsStream(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    public static Enumeration<URL> getSystemResources(String name) throws IOException {
        throw new UnsupportedOperationException("TODO");
    }

    public final ClassLoader getParent() {
        return this.parent;
    }    

    public static ClassLoader getSystemClassLoader() {
        return SystemClassLoader.systemLoader;
    }

    protected Package definePackage(String name, String specificationTitle, String specificationVersion, String specificationVender, String implementationTitle, String implementationVersion, String implementationVendor, URL sealBase) {
        throw new UnsupportedOperationException("TODO");
    }

    protected Package getPackage(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    protected Package[] getPackages() {
        throw new UnsupportedOperationException("TODO");
    }

    protected String findLibrary(String libraryName) {
        throw new UnsupportedOperationException("TODO");
    }

    public void clearAssertionStatus() {
        throw new UnsupportedOperationException("TODO");
    }

    public void setDefaultAssertionStatus(boolean enabled) {
        throw new UnsupportedOperationException("TODO");
    }

    public void setPackageAssertionStatus(String packageName, boolean enabled) {
        throw new UnsupportedOperationException("TODO");
    }

    public void setClassAssertionStatus(String className, boolean enabled) {
        throw new UnsupportedOperationException("TODO");
    }
}