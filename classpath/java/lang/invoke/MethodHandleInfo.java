package java.lang.invoke;

import java.lang.reflect.Member;

public interface MethodHandleInfo {
    public static final int REF_getField = 1;
    public static final int REF_getStatic = 2;
    public static final int REF_putField = 3;
    public static final int REF_putStatic = 4;
    public static final int REF_invokeVirtual = 5;
    public static final int REF_invokeStatic = 6;
    public static final int REF_invokeSpecial = 7;
    public static final int REF_newInvokeSpecial = 8;
    public static final int REF_invokeInterface = 9;
    
    public Class<?> getDeclaringClass();
    
    public String getName();
    
    public MethodType getMethodType();
    
    public<T extends Member> T reflectAs(Class<T> expected, MethodHandles.Lookup lookup);
    
    public int getModifiers();
    
    public int getReferenceKind();
    
    public default boolean isVarArgs() {
        throw new UnsupportedOperationException("TODO");
    }
    
    public static String referenceKindToString(int kind) {
        throw new UnsupportedOperationException("TODO");
    }
    
    public static String toString(int kind, Class<?> defClass, String name, MethodType type) {
        throw new UnsupportedOperationException("TODO");
    }
}