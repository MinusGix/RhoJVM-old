package sun.misc;

// This class exists so that if people are using RhoJVM with a jdk that has this class for internal 
// features, it won't load the actual class as that tends to rely on native JVM implementation
// details which won't hold for ours.
// We do not implement features on this class. It is here so that any function calls will meet
// errors rather than actually doing anything.

// TODO: We could have custom annotations / checks in the jvm to warn that a class tried using this // and it is not supported?

public class VM {
    public static boolean isBooted() {
        return true;
    }

    public static long maxDirectMemory() {
        return 128 * 1024 * 1024;
    }
}