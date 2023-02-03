package java.lang;

import java.util.function.Supplier;
import java.lang.Thread;

public class ThreadLocal<T> {
    public ThreadLocal() {}

    public static<S> ThreadLocal<S> withInitial(Supplier<S> supplier) {
        throw new UnsupportedOperationException();
    }

    public native T get();

    public native void set(T value);

    public native void remove();

    protected T initialValue() {
        return null;
    }
}