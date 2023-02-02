package rho.util;

// Class for logging information from within Java.  
// This lets you avoid using `System.out.println`, and it puts it inline with the rest of the 
// logging.  
// Obviously, users should not use this themselves since it is not cross-jvm compatible.
public class Log {
    public static native void info(String msg);

    public static native void warn(String msg);

    public static native void error(String msg);
}