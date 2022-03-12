package java.lang;

import java.lang.reflect.AnnotatedElement;
import java.lang.annotation.Annotation;
import java.net.URL;

public class Package implements AnnotatedElement {
    private String name;

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
        return null;
    }

    public String getImplementationVendor() {
        return null;
    }

    public String getImplementationVersion() {
        return null;
    }

    public String getSpecificationTitle() {
        return null;
    }

    public String getSpecificationVendor() {
        return null;
    }

    public String getSpecificationVersion() {
        return null;
    }

    public boolean isSealed() {
        return false;
    }

    public boolean isSealed(URL url) {
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