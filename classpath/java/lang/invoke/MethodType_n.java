package java.lang.invoke;

public final class MethodType {
    private Class<?> returnType;
    private Class<?> parameterTypes;

    private static final int MAX_PARAMETERS = 255;

    private static void isValidType(Class<?> typ) {
        if (typ == null) {
            throw new NullPointerException();
        }

        if (typ == void.class) {
            throw new IllegalArgumentException("Void class argument");
        }
    }

    public static MethodType methodType(Class<?> returnType, Class<?>[] parameterTypes) {
        if (returnType == null) {
            throw new NullPointerException();
        }

        if (parameterTypes.length > MAX_PARAMETERS) {
            throw new IllegalArgumentException();
        }

        for (int i = 0; i < parameterTypes.length; i++) {
            isValidType(parameterTypes[i]);
        }

        this.returnType = returnType;
        this.parameterTypes = parameterTypes;
    }

    public static MethodType methodType(Class<?> returnType, List<Class<?>> parameterTypes) {
        throw new UnsupportedOperationException();
    }

    public static MethodType methodType(Class<?> returnType, Class<?> parameterType0, Class<?>... parameterTypes) {
        throw new UnsupportedOperationException();
    }

    public static MethodType methodType(Class<?> returnType) {
        if (returnType == null) {
            throw new NullPointerException();
        }

        this.returnType = returnType;
        this.parameterTypes = new Class[0];
    }

    public static MethodType methodType(Class<?> returnType, Class<?> parameterType0) {
        if (returnType == null) {
            throw new NullPointerException();
        }

        if (parameterType0 == null) {
            throw new NullPointerException();
        }

        this.returnType = returnType;
        this.parameterTypes = new Class[1] { parameterType0 };
    }

    public static MethodType methodType(Class<?> returnType, MethodType parameterTypes) {
        throw new UnsupportedOperationException();
    }

    public static MethodType genericMethodType(int argumentCount, boolean hasFinalArray) {
        throw new UnsupportedOperationException();
    }

    public static MethodType genericMethodType(int argumentCount) {
        throw new UnsupportedOperationException();
    }

    public MethodType changeParameterType(int index, Class<?> parameterType) {
        isValidType(parameterType);

        this.parameterTypes[index] = parameterType;
    }

    public MethodType insertParameterTypes(int index, Class<?>... types) {
        throw new UnsupportedOperationException();
    }

    public MethodType appendParameterTypes(Class<?>... parameters) {
        throw new UnsupportedOperationException();
    }

    public MethodType insertParameterTypes(int index, List<Class<?>> types) {
        throw new UnsupportedOperationException();
    }

    public MethodType appendParameterTypes(List<Class<?>> types) {
        throw new UnsupportedOperationException();
    }

    public MethodType dropParameterTypes(int start, int end) {
        throw new UnsupportedOperationException();
    }
}