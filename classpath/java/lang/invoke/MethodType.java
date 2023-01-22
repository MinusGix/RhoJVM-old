package java.lang.invoke;

import java.util.List;
import java.util.Arrays;

public final class MethodType {
    private Class<?> returnTy;
    private Class<?>[] paramTys;

    private static final int MAX_PARAMETERS = 255;

    private static void isValidType(Class<?> ty) {
        if (ty == null) {
            throw new NullPointerException();
        }

        if (ty == void.class) {
            throw new IllegalArgumentException("Void class argument");
        }
    }

    private MethodType () {
        this.returnTy = void.class;
        this.paramTys = new Class[0];
    }

    public static MethodType methodType(Class<?> returnTy, Class<?>[] paramTys) {
        if (returnTy == null) {
            throw new NullPointerException();
        }

        if (paramTys.length > MAX_PARAMETERS) {
            throw new IllegalArgumentException();
        }

        for (int i = 0; i < paramTys.length; i++) {
            isValidType(paramTys[i]);
        }

        MethodType inst = new MethodType();

        inst.returnTy = returnTy;
        inst.paramTys = paramTys;

        return inst;
    }

    public static MethodType methodType(Class<?> returnTy, List<Class<?>> paramTys) {
        if (returnTy == null) {
            throw new NullPointerException();
        }

        if (paramTys.size() > MAX_PARAMETERS) {
            throw new IllegalArgumentException();
        }

        for (int i = 0; i < paramTys.size(); i++) {
            isValidType(paramTys.get(i));
        }

        MethodType inst = new MethodType();

        inst.returnTy = returnTy;
        inst.paramTys = paramTys.toArray(new Class[0]);

        return inst;
    }

    public static MethodType methodType(Class<?> returnTy, Class<?> paramTy, Class<?>... paramTys) {
        if (returnTy == null) {
            throw new NullPointerException();
        }

        isValidType(paramTy);

        // TODO: make this resistant to overflow?
        if (paramTys.length + 1 > MAX_PARAMETERS) {
            throw new IllegalArgumentException();
        }

        for (int i = 0; i < paramTys.length; i++) {
            isValidType(paramTys[i]);
        }

        MethodType inst = new MethodType();

        inst.returnTy = returnTy;
        inst.paramTys = new Class[paramTys.length + 1];
        inst.paramTys[0] = paramTy;
        for (int i = 0; i < paramTys.length; i++) {
            inst.paramTys[i + 1] = paramTys[i];
        }

        return inst;
    }

    public static MethodType methodType(Class<?> returnTy) {
        if (returnTy == null) {
            throw new NullPointerException();
        }

        MethodType inst = new MethodType();

        inst.returnTy = returnTy;
        inst.paramTys = new Class[0];

        return inst;
    }

    public static MethodType methodType(Class<?> returnTy, Class<?> paramTy) {
        if (returnTy == null) {
            throw new NullPointerException();
        }

        if (paramTy == null) {
            throw new NullPointerException();
        }

        MethodType inst = new MethodType();

        inst.returnTy = returnTy;
        inst.paramTys = new Class[] { paramTy };

        return inst;
    }

    public static MethodType methodType(Class<?> returnTy, MethodType paramTys) {
        throw new UnsupportedOperationException();
    }

    public static MethodType genericMethodType(int count, boolean hasFinalArray) {
        throw new UnsupportedOperationException();
    }

    public static MethodType genericMethodType(int count) {
        throw new UnsupportedOperationException();
    }

    // === Parameter ===
    public int parameterCount() {
        return this.paramTys.length;
    }

    public Class<?> parameterType(int idx) {
        return this.paramTys[idx];
    }

    public List<Class<?>> parameterList() {
        return Arrays.asList(this.paramTys.clone());
    }

    public Class<?>[] parameterArray() {
        return this.paramTys.clone();
    }

    public MethodType changeParameterType(int idx, Class<?> paramTy) {
        isValidType(paramTy);

        MethodType inst = new MethodType();

        inst.returnTy = this.returnTy;
        inst.paramTys = this.paramTys.clone();
        inst.paramTys[idx] = paramTy;

        return inst;
    }

    public MethodType insertParameterTypes(int idx, Class<?>... paramTys) {
        throw new UnsupportedOperationException();
    }

    public MethodType appendParameterTypes(Class<?>... paramTys) {
        throw new UnsupportedOperationException();
    }

    public MethodType insertParameterTypes(int idx, List<Class<?>> paramTys) {
        throw new UnsupportedOperationException();
    }

    public MethodType appendParameterTypes(List<Class<?>> paramTys) {
        throw new UnsupportedOperationException();
    }

    public MethodType dropParameterTypes(int start, int end) {
        throw new UnsupportedOperationException();
    }

    /// === Return ===
    public Class<?> returnType() {
        return this.returnTy;
    }

    public MethodType changeReturnType(Class<?> returnTy) {
        if (returnTy == null) {
            throw new NullPointerException();
        }

        MethodType inst = new MethodType();

        inst.returnTy = returnTy;
        inst.paramTys = this.paramTys.clone();

        return inst;
    }

    // === Modification Functions ===
    public MethodType erase() {
        throw new UnsupportedOperationException();
    }

    public MethodType generic() {
        throw new UnsupportedOperationException();
    }

    // === Information ==
    public boolean hasPrimitives() {
        throw new UnsupportedOperationException();
    }

    public boolean hasWrappers() {
        throw new UnsupportedOperationException();
    }


    // === Other ===
    public MethodType wrap() {
        throw new UnsupportedOperationException();
    }

    public MethodType unwrap() {
        throw new UnsupportedOperationException();
    }

    public native String toMethodDescriptorString();
}