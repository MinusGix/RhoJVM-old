package java.lang.invoke;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodType;
import java.lang.invoke.CallSite;

public class VolatileCallSite extends CallSite {
    MethodHandle target;

    public VolatileCallSite(MethodHandle target) {
        this.target = target;
    }

    @Override
    public MethodHandle dynamicInvoker() {
        // TODO
        throw new UnsupportedOperationException();
    }

    @Override
    public MethodHandle getTarget() {
        return this.target;
    }

    public void setTarget(MethodHandle newTarget) {
        throw new UnsupportedOperationException();
    }
}