package rho.invoke;

import java.lang.invoke.MethodHandleInfo;
import java.lang.invoke.MethodType;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.Member;

// NOTE: This class is assumed to have zero fields by the jvm to make it cheaper/simpler to 
// construct!
public class MethodHandleInfoInst implements MethodHandleInfo {
    public native Class<?> getDeclaringClass();
    
    public String getName() {
        throw new UnsupportedOperationException("TODO");
    }
    
    public native MethodType getMethodType();
    
    public native int getReferenceKind();
    
    public<T extends Member> T reflectAs(Class<T> expected, MethodHandles.Lookup lookup) {
        throw new UnsupportedOperationException("TODO");
    }
    
    public int getModifiers() {
        throw new UnsupportedOperationException("TODO");
    }
}