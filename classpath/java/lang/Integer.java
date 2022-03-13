package java.lang;

// FIXME: We need to cache values between -128 to 127 for valueOf!
public final class Integer extends Number implements Comparable<Integer> {
    public static final int MIN_VALUE = 0x80000000;
    public static final int MAX_VALUE = 0x7FFFFFFF;

    public static final int SIZE = 32;

    public static final int BYTES = 4;

    public static final Class<Integer> TYPE = (Class<Integer>) Class.getPrimitiveClass("I");

    private final int value;

    public Integer(int value) {
        this.value = value;
    }

    public Integer(String value) throws NumberFormatException {
        this.value = Integer.parseInt(value);
    }

    public static Integer valueOf(int value) {
        return new Integer(value);
    }

    public static Integer valueOf(String value) throws NumberFormatException {
        return new Integer(value);
    }

    public static Integer valueOf(String value, int radix) throws NumberFormatException {
        return new Integer(Integer.parseInt(value, radix));
    }

    public static int compare(int left, int right) {
        if (left == right) {
            return 0;
        } else if (left < right) {
            return -1;
        } else {
            return 1;
        }
    }

    public static int hashCode(int value) {
        return value;
    }

    public static String toString(int value) {
        return Integer.toString(value, 10);
    }

    public static native String toString(int value, int radix);

    public static String toBinaryString(int value) {
        return Integer.toString(value, 2);
    }

    public static String toHexString(int value) {
        return Integer.toString(value, 16);
    }

    public static String toOctalString(int value) {
        return Integer.toString(value, 8);
    }

    public static int parseInt(String source) throws NumberFormatException {
        return Integer.parseInt(source, 10);
    }

    public static native int parseInt(String source, int radix) throws NumberFormatException;

    public static Integer decode(String value) throws NumberFormatException {
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
        int result;
        if (value.startsWith("0x", cur) || value.startsWith("0X", cur)) {
            cur += 2;
            result = Integer.parseInt(value.substring(cur), 16);
        } else if (value.startsWith("#", cur)) {
            cur += 1;
            result = Integer.parseInt(value.substring(cur), 16);
        } else if (value.startsWith("0", cur) && cur + 1 < value.length()) {
            cur += 1;
            result = Integer.parseInt(value.substring(cur, 8));
        } else {
            wasDefault = true;
            result = Integer.parseInt(value.substring(cur, 10));
        }

        if (!wasDefault && (value.startsWith("+", cur) || value.startsWith("-", cur))) {
            throw new NumberFormatException("Invalid sign character location, it should go at the start of the number.");
        }

        if (isNegative) {
            return new Integer(-result);
        } else {
            return new Integer(result);
        }
    }

    public static Integer getInteger(String propertyName) {
        return Integer.getInteger(propertyName, null);
    }

    public static Integer getInteger(String propertyName, int defaultValue) {
        return Integer.getInteger(propertyName, new Integer(defaultValue));
    }

    public static Integer getInteger(String propertyName, Integer defaultValue) {
        String value = null;
        try {
            value = System.getProperty(propertyName);
        } catch (NullPointerException | IllegalArgumentException err) {}

        if (value != null) {
            try {
                return Integer.decode(value);
            } catch (NumberFormatException err) {}
        }

        return defaultValue;
    }

    public static int signum(int value) {
        if (value == 0) {
            return 0;
        } else if (value > 0) {
            return 1;
        } else {
            return -1;
        }
    }

    // TODO:
    // public static int bitCount(int value) {}

    // public static int highestOneBit(int value) {}

    // public static int lowestOneBit(int value) {}
    
    // public static int number numberOfLeadingZeros(int value) {}

    // public static int numberOfTrailingZeroes(int value) {}

    // public static int reverse(int value) {}

    // public static int reverseBytes(int value) {}

    // public static int rotateLeft(int value, int distance) {}

    // public static int rotateRight(int value, int distance) {}

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

    public int compareTo(Integer other) {
        return Integer.compare(this.value, other.value);
    }

    public int hashCode() {
        return Integer.hashCode(this.value);
    }

    public String toString() {
        return Integer.toString(this.value);
    }

    public boolean equals(Object other) {
        if (other instanceof Integer) {
            return value == ((Integer) other).value;
        }

        return false;
    }
}