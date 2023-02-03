package java.lang;

import java.util.function.Supplier;
import java.lang.Thread;

import rho.SupplierThreadLocal;

public class ThreadLocal<T> {
    public ThreadLocal() {}

    public static<S> ThreadLocal<S> withInitial(Supplier<S> supplier) {
        return new SupplierThreadLocal<S>(supplier);
    }

    public native T get();

    public native void set(T value);

    public native void remove();

    protected T initialValue() {
        return null;
    }
}