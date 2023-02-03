package rho;

import java.util.function.Supplier;

public class SupplierThreadLocal<T> extends ThreadLocal<T> {
    private final java.util.function.Supplier<T> supplier;

    public SupplierThreadLocal(java.util.function.Supplier<T> supplier) {
        this.supplier = supplier;
    }

    @Override
    protected T initialValue() {
        return supplier.get();
    }
}