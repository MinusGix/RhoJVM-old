package rho.invoke;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodType;

// NOTE: This class is assumed to have zero fields by the jvm to make it cheaper/simpler to 
// construct!
public final class MethodHandleInst extends MethodHandle {
    private MethodHandleInst() {}

    public native Object invoke(Object... args);

    public native MethodType type();
}