package java.lang;

public final class Long extends Number implements Comparable<Long> {
    public static final long MIN_VALUE = 0x8000000000000000L;
    public static final long MAX_VALUE = 0x7fffffffffffffffL;

    public static final int SIZE = 64;
    public static final int bytes = 8;

    public static final Class<Long> TYPE = (Class<Long>) Class.getPrimitiveClass("J");

    private final long value;

    public Long(long value) {
        this.value = value;
    }

    public Long(String value) throws NumberFormatException {
        this.value = Long.parseLong(value);
    }

    public static Long valueOf(long value) {
        return new Long(value);
    }

    public static Long valueOf(String value) throws NumberFormatException {
        return new Long(value);
    }

    public static Long valueOf(String value, int radix) throws NumberFormatException {
        return new Long(Long.parseLong(value, radix));
    }

    public static int compare(long left, long right) {
        if (left == right) {
            return 0;
        } else if (left < right) {
            return -1;
        } else {
            return 1;
        }
    }

    public static int hashCode(long value) {
        return (int)(value ^ (value >>> 32));
    }

    public static String toString(long value) {
        return Long.toString(value, 10);
    }

    public static native String toString(long value, int radix);

    public static String toBinaryString(long value) {
        return Long.toString(value, 2);
    }

    public static String toHexString(long value) {
        return Long.toString(value, 16);
    }

    public static String toOctalString(long value) {
        return Long.toString(value, 8);
    }

    public static long parseLong(String source) throws NumberFormatException {
        return Long.parseLong(source, 10);
    }

    public static native long parseLong(String source, int radix) throws NumberFormatException;

    public static Long decode(String value) throws NumberFormatException {
        if (value.isEmpty()) {
            throw new NumberFormatException("Empty string");
        }

        boolean isNegative = false;
        int cur = 0;

        char start = value.charAt(0);
        if (start == '+') {
            // isNegative = false;
            cur++;
        } else if (start == '-') {
            isNegative = true;
            cur++;
        }

        boolean wasDefault = false;
        long result;
        if (value.startsWith("0x", cur) || value.startsWith("0X", cur)) {
            cur += 2;
            result = Long.parseLong(value.substring(cur), 16);
        } else if (value.startsWith("#", cur)) {
            cur += 1;
            result = Long.parseLong(value.substring(cur), 16);
        } else if (value.startsWith("0", cur) && cur + 1 < value.length()) {
            cur += 1;
            result = Long.parseLong(value.substring(cur, 8));
        } else {
            wasDefault = true;
            result = Long.parseLong(value.substring(cur, 10));
        }

        if (!wasDefault && (value.startsWith("+", cur) || value.startsWith("-", cur))) {
            throw new NumberFormatException("Invalid sign character location, it should go at the start of the number.");
        }

        if (isNegative) {
            return new Long(-result);
        } else {
            return new Long(result);
        }
    }

    public static Long getLong(String propertyName) {
        return Long.getLong(propertyName, null);
    }

    public static Long getLong(String propertyName, long defaultValue) {
        return Long.getLong(propertyName, new Long(defaultValue));
    }

    public static Long getLong(String propertyName, Long defaultValue) {
        String value = null;
        try {
            value = System.getProperty(propertyName);
        } catch (NullPointerException | IllegalArgumentException err) {}

        if (value != null) {
            try {
                return Long.decode(value);
            } catch (NumberFormatException err) {}
        }

        return defaultValue;
    }

    // === Operations ===
    public native static int numberOfLeadingZeros(long value);

    public static int signum(long value) {
        if (value == 0) {
            return 0;
        } else if (value > 0) {
            return 1;
        } else {
            return -1;
        }
    }

    public static long sum(long a, long b) {
        return a + b;
    }

    public static long max(long a, long b) {
        return Math.max(a, b);
    }

    public static long min(long a, long b) {
        return Math.min(a, b);
    }

    // TODO: Parse unsigned long
    // TODO: Bunch of other functions

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

    public int compareTo(Long other) {
        return Long.compare(this.value, other.value);
    }

    public int hashCode() {
        return Long.hashCode(this.value);
    }

    public String toString() {
        return Long.toString(this.value);
    }

    public boolean equals(Object other) {
        if (other instanceof Long) {
            return value == ((Long) other).value;
        }
        
        return false;
    }
}