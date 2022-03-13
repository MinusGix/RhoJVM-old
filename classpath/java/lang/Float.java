package java.lang;

public final class Float extends Number {
    public static final float MAX_VALUE = 3.40282346638528859812e+38f;
    public static final float MIN_VALUE = 1.17549435082228750797e-38f;

    public static final float NEGATIVE_INFINITY = -1.0f / 0.0f;
    public static final float POSITIVE_INFINITY = 1.0f / 0.0f;
    public static final float NaN = 0.0f / 0.0f;

    public static final float MIN_NORMAL = 0x1.0p-126f;

    public static final int MAX_EXPONENT = 127;
    public static final int MIN_EXPONENT = -126;

    public static final int SIZE = 32;
    public static final int BYTES = 4;

    public static final Class<Float> TYPE = (Class<Float>) Class.getPrimitiveClass("F");
    
    private static final int EXPONENT_BIT_MASK = 0x7F800000;
    private static final int SIGNIFICAND_BIT_MASK = 0x007FFFFF;

    private final float value;

    public Float(float value) {
        this.value = value;
    }

    public Float(double value) {
        this.value = (float)value;
    }

    public Float(String value) throws NumberFormatException {
        this.value = Float.parseFloat(value);
    }

    public static native String toString(float value);

    public static int hashCode(float value) {
        return Float.floatToIntBits(value);
    }

    public static Float valueOf(float value) {
        return new Float(value);
    }

    public static Float valueOf(String value) throws NumberFormatException {
        return new Float(Float.parseFloat(value));
    }

    public static native float parseFloat(String value) throws NumberFormatException;

    public static boolean isNaN(float val) {
        return val != val;
    }

    public static boolean isInfinite(float val) {
        return val == POSITIVE_INFINITY || val == NEGATIVE_INFINITY;
    }

    public static boolean isFinite(float val) {
        return !Float.isNaN(val) && !Float.isInfinite(val);
    }

    public static int floatToIntBits(float value) {
        int raw = Float.floatToRawIntBits(value);

        if (((raw & EXPONENT_BIT_MASK) == EXPONENT_BIT_MASK) && (raw & SIGNIFICAND_BIT_MASK) != 0) {
            // NaN
            return 0x7FC00000;
        } else {
            return raw;
        }
    }

    public static native int floatToRawIntBits(float value);

    public static native float intBitsToFloat(int raw);

    public boolean isNaN() {
        return Float.isNaN(this.value);
    }

    public boolean isInfinite() {
        return Float.isInfinite(this.value);
    }

    public byte byteValue() {
        return (byte)this.value;
    }

    public short shortValue() {
        return (short)this.value;
    }

    public int intValue() {
        return (int)this.value;
    }

    public long longValue() {
        return (long)this.value;
    }

    public float floatValue() {
        return (float)this.value;
    }

    public double doubleValue() {
        return (double)this.value;
    }

    public int hashCode() {
        return Float.hashCode(this.value);
    }

    public String toString() {
        return Float.toString(this.value);
    }

    public boolean equals(Object other) {
        if (other instanceof Float) {
            return this.value == ((Float)other).value;
        }
        return false;
    }
}