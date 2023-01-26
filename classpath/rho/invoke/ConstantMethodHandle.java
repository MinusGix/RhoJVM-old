package rho.invoke;

import java.lang.invoke.MethodHandle;

public class ConstantMethodHandle extends MethodHandle {
    private final Object value;

    public ConstantMethodHandle(Object value) {
        this.value = value;
    }

    public Object invoke(Object... args) {
        return value;
    }

    // TODO: Other invoke functions
}