package java.lang.invoke;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodType;

public abstract class CallSite {
    public abstract MethodHandle dynamicInvoker();

    public abstract MethodHandle getTarget();

    public abstract void setTarget(MethodHandle newTarget);

    public MethodType type() {
        return getTarget().type();
    }
}