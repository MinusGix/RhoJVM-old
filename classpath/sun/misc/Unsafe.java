package sun.misc;

import java.lang.reflect.Field;

public final class Unsafe {
    private void Unsafe () {}

    private static final Unsafe instance = new Unsafe();

    public static Unsafe getUnsafe() {
        return instance;
    }

    public native long allocateMemory(long bytes);

    public void setMemory(Object base, long offset, long count, byte value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public native void freeMemory(long address);

    public native byte getByte(long address);

    public native void putByte(long address, byte value);

    public native short getShort(long address);

    public native void putShort(long address, short value);

    public native char getChar(long address);

    public native void putChar(long address, char value);

    public native int getInt(long address);

    public native void putInt(long address, int value);

    public native long getLong(long address);

    public native void putLong(long address, long value);

    public native float getFloat(long address);

    public native void putFloat(long address, float value);

    public native double getDouble(long address);

    public native void putDouble(long address, double x);

    public boolean getBooleanVolatile(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putBooleanVolatile(Object o, long offset, boolean value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public byte getByteVolatile(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putByteVolatile(Object o, long offset, byte value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public short getShortVolatile(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putShortVolatile(Object o, long offset, short value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public char getCharVolatile(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putCharVolatile(Object o, long offset, char value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public int getIntVolatile(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putIntVolatile(Object o, long offset, int value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public float getFloatVolatile(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putFloatVolatile(Object o, long offset, float value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public double getDoubleVolatile(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putDoubleVolatile(Object o, long offset, double value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public long getLongVolatile(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putLongVolatile(Object o, long offset, long value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public int getInt(Object o, long offset) {
        return this.getIntField(o, offset);
    }

    public void putInt(Object o, long offset, int value) {
        this.putIntField(o, offset, value);
    }

    // This is currently just like this so we don't have to implement native overloading support
    private native int getIntField(Object o, long offset);
    private native void putIntField(Object o, long offset, int value);

    public long getLong(Object o, long offset) {
        return this.getLongField(o, offset);
    }

    public void putLong(Object o, long offset, long value) {
        this.putLongField(o, offset, value);
    }

    private native long getLongField(Object o, long offset);
    private native void putLongField(Object o, long offset, long value);

    public double getDouble(Object o, long offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putDouble(Object o, long offset, double value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putOrderedLong(Object o, long offset, long value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putOrderedInt(Object o, long offset, int value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Object getObject(Object o, long offset) {
        return this.getObjectField(o, offset);
    }

    public void putObject(Object o, long offset, Object value) {
        this.putObjectField(o, offset, value);
    }

    private native Object getObjectField(Object o, long offset);
    private native void putObjectField(Object o, long offset, Object value);

    public void putObjectVolatile(Object o, long offset, Object value) {
        // FIXME: This isn't volatile
        this.putObjectField(o, offset, value);
    }

    public void putOrderedObject(Object o, long offset, Object value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public Object getObjectVolatile(Object o, long offset) {
        // FIXME: This isn't volatile
        return this.getObjectField(o, offset);
    }

    public long getAddress(long address) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void putAddress(long address, long value) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public int arrayBaseOffset(Class arrayClass) {
        // We do a somewhat amusing thing here, we just return 0 as the base address
        // So then when they use it on an array, we just assume that the offset they've given is to 
        // access the index.
        // However, once we start doing instance compression, we can give them real memory offset 
        // into our object, to make the accesses very direct.
        return 0;
    }

    public int arrayIndexScale(Class arrayClass) {
        return 1;
    }

    // Implemented in jvm
    public native long objectFieldOffset(Field field);

    public void park(boolean absolute, long time) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }
    
    public void unpark(Object target) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void copyMemory(Object srcBase, long srcOffset, Object destBase, long destOffset, long count) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public final native int getAndAddInt(Object src, long offset, int delta);

    public boolean compareAndSwapInt(Object o, long offset, int old, int newVal) {
        // FIXME: This is obviously not an actual cas
        int current = this.getInt(o, offset);
        if (current == old) {
            this.putInt(o, offset, newVal);
            return true;
        } else {
            return false;
        }
    }

    public boolean compareAndSwapLong(Object o, long offset, long old, long newVal) {
        // FIXME: This is obviously not an actual cas
        long current = this.getLong(o, offset);
        if (current == old) {
            this.putLong(o, offset, newVal);
            return true;
        } else {
            return false;
        }
    }

    public boolean compareAndSwapObject(Object o, long offset, Object old, Object newVal) {
        // FIXME: This is obviously not an actual cas
        Object current = this.getObject(o, offset);
        if (current == old) {
            this.putObject(o, offset, newVal);
            return true;
        } else {
            return false;
        }
    }

    public void copyMemory(long src, long dst, long count) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void throwException(Throwable t) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public native Class<?> defineAnonymousClass(Class<?> baseClass, byte[] data, Object[] patches);
}