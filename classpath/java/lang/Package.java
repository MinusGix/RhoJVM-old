package java.lang;

import java.lang.reflect.AnnotatedElement;
import java.lang.annotation.Annotation;
import java.net.URL;

public class Package implements AnnotatedElement {
    private final String name;
    
    private final String specTitle;
    private final String specVendor;
    private final String specVersion;

    private final String implTitle;
    private final String implVendor;
    private final String implVersion;

    private final boolean isSealed;

    // Used by the jvm
    private Package(
        String name,
        String specTitle,
        String specVendor,
        String specVersion,
        String implTitle,
        String implVendor,
        String implVersion,
        boolean isSealed
    ) {
        this.name = name;
        this.specTitle = specTitle;
        this.specVendor = specVendor;
        this.specVersion = specVersion;
        this.implTitle = implTitle;
        this.implVendor = implVendor;
        this.implVersion = implVersion;
        this.isSealed = isSealed;
    }

    public static Package getPackage(String name) {
        throw new UnsupportedOperationException("TODO");
    }

    public static Package[] getPackages() {
        throw new UnsupportedOperationException("TODO");
    }

    public String getName() {
        return this.name;
    }

    // TODO: implement these.
    public String getImplementationTitle() {
        return this.implTitle;
    }

    public String getImplementationVendor() {
        return this.implVendor;
    }

    public String getImplementationVersion() {
        return this.implVersion;
    }

    public String getSpecificationTitle() {
        return this.specTitle;
    }

    public String getSpecificationVendor() {
        return this.specVendor;
    }

    public String getSpecificationVersion() {
        return this.specVersion;
    }

    public boolean isSealed() {
        return this.isSealed;
    }

    public boolean isSealed(URL url) {
        // TODO: ?
        return false;
    }

    public boolean isCompatibleWith(String target) {
        throw new UnsupportedOperationException("TODO");
    }

    public int hashCode() {
        return this.name.hashCode();
    }

    public String toString() {
        return this.name;
    }

    // Annotations

    public<A extends Annotation> A getAnnotation(Class<A> clazz) {
        throw new UnsupportedOperationException("TODO");
    }

    public Annotation[] getAnnotations() {
        throw new UnsupportedOperationException("TODO");
    }

    public Annotation[] getDeclaredAnnotations() {
        throw new UnsupportedOperationException("TODO");
    }
}