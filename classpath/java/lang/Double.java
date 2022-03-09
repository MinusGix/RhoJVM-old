package java.lang;

// TODO: implement comparable?
public final class Double extends Number {
    public static final double MAX_VALUE = 0x1.fffffffffffffP+1023;
    public static final double MIN_VALUE = 0x0.0000000000001P-1022;

    public static final double NEGATIVE_INFINITY = -1.0 / 0.0;
    public static final double POSITIVE_INFINITY = 1.0 / 0.0;
    public static final double NaN = 0.0 / 0.0;

    public static final double MIN_NORMAL = 0x1.0p-1022;

    public static final int MAX_EXPONENT = 1023;
    public static final int MIN_EXPONENT = -1022;

    public static final int SIZE = 64;
    public static final int BYTES = 8;

    public static final Class<Double> TYPE = (Class<Double>) Class.getPrimitive('D');
    
    private static final long EXPONENT_BIT_MASK = 0x7FF0000000000000L;
    private static final long SIGNIFICAND_BIT_MASK = 0x000FFFFFFFFFFFFFL;

    private final double value;

    public Double(double value) {
        this.value = value;
    }

    public Double(String value) {
        this.value = Double.parseDouble(value);
    }

    public static native String toString(double value);

    public static int hashCode(double value) {
        long raw = Double.doubleToLongBits(value);
        return (int) (raw ^ (raw & 0xFF));
    }

    public static Double valueOf(double value) {
        return new Double(value);
    }

    public static Double valueOf(String value) {
        return new Double(value);
    }

    public static native double parseDouble(String value);

    public static boolean isNaN(double val) {
        return val != val;
    }

    public static boolean isInfinite(double val) {
        return val == Double.POSITIVE_INFINITY || val == Double.NEGATIVE_INFINITY;
    }

    public static boolean isFinite(double val) {
        return !Double.isNaN(val) && !Double.isInfinite(val);
    }

    public static long doubleToLongBits(double value) {
        long raw = Double.doubleToRawLongBits(value);
        if ((raw & EXPONENT_BIT_MASK) == EXPONENT_BIT_MASK && (raw & SIGNIFICAND_BIT_MASK) != 0L) {
            // NaN
            return 0x7ff8000000000000L;
        } else {
            return raw;
        }
    }

    public static native long doubleToRawLongBits(double value);

    public static native double longBitsToDouble(long raw);

    public boolean isNaN() {
        return Double.isNaN(this.value);
    }

    public boolean isInfinite() {
        return Double.isInfinite(this.value);
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
        return Double.hashCode(this.value);
    }

    public String toString() {
        return Double.toString(this.value);
    }

    public boolean equals(Object other) {
        if (other instanceof Double) {
            return this.value == ((Double)other).value;
        }

        return false;
    }
}