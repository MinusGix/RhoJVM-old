package java.lang.reflect;

public final class Array {
    public static Object get(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    // TODO: Can we just implement these getPrimitive functions as a checked cast an array of their 
    // given types? Though, we also need to do types that can expand, so indexing an int[] can work
    // for a function that returns a long
    public static boolean getBoolean(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    public static byte getByte(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    public static char getChar(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    public static short getShort(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    public static int getInt(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    public static long getLong(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    public static float getFloat(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    public static byte getDouble(Object array, int index) {
        throw new UnsupportedOperationException();
    }

    public static void set(Object array, int index, Object value) {
        throw new UnsupportedOperationException();
    }

    public static void setBoolean(Object array, int index, boolean value) {
        throw new UnsupportedOperationException();
    }

    public static void setByte(Object array, int index, byte value) {
        throw new UnsupportedOperationException();
    }

    public static void setChar(Object array, int index, char value) {
        throw new UnsupportedOperationException();
    }

    public static void setShort(Object array, int index, short value) {
        throw new UnsupportedOperationException();
    }

    public static void setInt(Object array, int index, int value) {
        throw new UnsupportedOperationException();
    }

    public static void setLong(Object array, int index, long value) {
        throw new UnsupportedOperationException();
    }

    public static void setFloat(Object array, int index, float value) {
        throw new UnsupportedOperationException();
    }

    public static void setDouble(Object array, int index, byte value) {
        throw new UnsupportedOperationException();
    }

    public static int getLength(Object array) {
        throw new UnsupportedOperationException();
    }

    public static Object newInstance(Class<?> component, int... dimensions) {
        throw new UnsupportedOperationException();
    }

    public static Object newInstance(Class<?> component, int length) {
        return Array.newInstanceArray(component, length);
    }

    private static native Object newInstanceArray(Class<?> component, int length);
}