package java.lang;

import rho.StringConversion;

import java.io.Serializable;
import java.util.Comparator;
import java.util.Formatter;
import java.util.Locale;
import java.util.regex.Pattern;
import java.nio.charset.Charset;
import java.nio.charset.CharacterCodingException;

public final class String implements java.io.Serializable, Comparable<String>, CharSequence {
    private final char data[];
    private int hash;

    public static Comparator<String> CASE_INSENSITIVE_ORDER = new Comparator<String>() {
        public int compare(String left, String right) {
            return left.compareToIgnoreCase(right);
        }
    };

    public String() {
        this.data = new char[0];
    }

    private String(char directSrc[], boolean _v) {
        this.data = directSrc;
    }

    private static String makeDirect(char src[]) {
        String val = new String(src, false);
        return val;
    }

    public String(String other) {
        this.data = other.data;
        this.hash = other.hash;
    }

    public String(char src[]) {
        int length = src.length;
        char[] dest = new char[length];
        System.arraycopy(src, 0, dest, 0, length);
        this.data = dest;
    }

    public String(char data[], int offset, int length) {
        int actualLength = data.length;
        if (offset < 0) {
            throw new StringIndexOutOfBoundsException(offset);
        } else if (length < 0) {
            throw new StringIndexOutOfBoundsException(length);
        } else if (actualLength <= length + offset) {
            throw new StringIndexOutOfBoundsException(length + offset);
        }

        char[] dest = new char[length];
        System.arraycopy(data, offset, dest, 0, length);
        this.data = dest;
    }

    public String(byte data[]) throws CharacterCodingException {
        this.data = StringConversion.convertToChars(data, 0, data.length, Charset.defaultCharset());
    }

    public String(byte data[], String charsetName) throws CharacterCodingException {
        Charset charset = Charset.forName(charsetName);
        this.data = StringConversion.convertToChars(data, 0, data.length, charset);
    }

    public String(byte data[], Charset charset) throws CharacterCodingException {
        this.data = StringConversion.convertToChars(data, 0, data.length, charset);
    }

    public String (byte data[], int offset, int length) throws CharacterCodingException, StringIndexOutOfBoundsException {
        int actualLength = data.length;
        if (offset < 0) {
            throw new StringIndexOutOfBoundsException(offset);
        } else if (length < 0) {
            throw new StringIndexOutOfBoundsException(length);
        } else if (actualLength <= length + offset) {
            throw new StringIndexOutOfBoundsException(length + offset);
        }

        this.data = StringConversion.convertToChars(data, offset, length, Charset.defaultCharset());
    }

    public String(byte data[], int offset, int length, String charsetName) throws CharacterCodingException {
        this(data, offset, length, Charset.forName(charsetName));
    }

    public String(byte data[], int offset, int length, Charset charset) throws CharacterCodingException {
        if (charset == null) {
            throw new NullPointerException("charset");
        }
        if (offset < 0) {
            throw new StringIndexOutOfBoundsException(offset);
        } else if (length < 0) {
            throw new StringIndexOutOfBoundsException(length);
        } else if (offset > data.length - length) {
            throw new StringIndexOutOfBoundsException(length + offset);
        }

        this.data = StringConversion.convertToChars(data, offset, length, charset);
    }

    public int length() {
        return data.length;
    }

    public boolean isEmpty() {
        return this.data.length == 0;
    }

    public char charAt(int index) {
        if (index < 0|| index >= this.data.length) {
            throw new StringIndexOutOfBoundsException(index);
        }

        return this.data[index];
    }

    public int codePointAt(int index) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public int codePointBefore(int index) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }
    
    public int codePointCount(int start, int end) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }
    
    public int offsetByCodePoints(int index, int offset) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public void getChars(int beginSource, int endSource, char dest[], int beginDest) {
        if (endSource > this.data.length) {
            throw new StringIndexOutOfBoundsException(endSource);
        } else if (beginSource > endSource) {
            throw new StringIndexOutOfBoundsException(endSource - beginSource);
        } else if (beginSource < 0) {
            throw new StringIndexOutOfBoundsException(beginSource);
        }

        System.arraycopy(this.data, beginSource, dest, beginDest, endSource - beginSource);
    }    

    // TODO: getBytes(beginSource, endSource, dest, beginDest)

    public byte[] getBytes() throws CharacterCodingException {
        return getBytes(Charset.defaultCharset());
    }

    public byte[] getBytes(String charsetName) throws CharacterCodingException  {
        if (charsetName == null) {
            throw new NullPointerException("charsetName");
        }

        Charset charset = Charset.forName(charsetName);
        return StringConversion.convertFromChars(this.data, 0, this.data.length, charset);
    }

    public byte[] getBytes(Charset charset) throws CharacterCodingException {
        if (charset == null) {
            throw new NullPointerException("charset");
        }

        return StringConversion.convertFromChars(this.data, 0, this.data.length, charset);
    }

    public String toLowerCase(Locale locale) {
        if (locale == null) {
            throw new NullPointerException("locale");
        }

        if (locale == Locale.ENGLISH) {
            return this.toLowerCase();
        } else {
            // TODO: I'm not sure how to implement this.
            throw new UnsupportedOperationException("TODO: Implement support for locales other than english");
        }
    }

    public String toLowerCase() {
        // TODO: Use default locale
        char[] output = new char[this.data.length];
        boolean changed = false;
        for (int i = 0; i < this.data.length; i++) {
            char lower = Character.toLowerCase(this.data[i]);
            if (lower != i) {
                changed = true;
            }
            output[i] = lower;
        }

        if (changed) {
            return String.makeDirect(output);
        } else {
            return this;
        }
    }

    public String toUpperCase(Locale locale) {
        if (locale == null) {
            throw new NullPointerException("locale");
        }

        if (locale == Locale.ENGLISH) {
            return this.toUpperCase();
        } else {
            // TODO: I'm not sure how to implement this.
            throw new UnsupportedOperationException("TODO: Implement support for locales other than english");
        }
    }

    public String toUpperCase() {
        // TODO: Use default locale
        char[] output = new char[this.data.length];
        boolean changed = false;
        for (int i = 0; i < this.data.length; i++) {
            char lower = Character.toUpperCase(this.data[i]);
            if (lower != i) {
                changed = true;
            }
            output[i] = lower;
        }

        if (changed) {
            return String.makeDirect(output);
        } else {
            return this;
        }
    }
    
    public boolean equals(Object other) {
        if (this == other) {
            return true;
        }

        if (!(other instanceof String)) {
            return false;
        }

        String otherS = (String) other;

        // Compare the lengths first, since if those don't match then they can't be equal
        if (this.data.length != otherS.data.length) {
            return false;
        }

        for (int i = 0; i < this.data.length; i++) {
            if (this.data[i] != otherS.data[i]) {
                return false;
            }
        }

        return true;
    }

    public boolean contentEquals(StringBuffer sb) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean contentEquals(CharSequence cs) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public boolean equalsIgnoreCase(String other) {
        if (this == other) {
            return true;
        }

        // TODO: Should this be a null pointer exception?
        if (other == null) {
            return false;
        }

        if (this.data.length != other.data.length) {
            return false;
        }

        return compareToIgnoreCase(other) == 0;
    }

    public int compareTo(String other) {
        if (this == other) {
            return 0;
        }

        int last = Math.min(this.data.length, other.data.length);
        for (int i = 0; i < last; i++) {
            char currentLeft = this.charAt(i);
            char currentRight = other.charAt(i);
            if (currentLeft - currentRight != 0) {
                return currentLeft - currentRight;
            }
        }

        return last - other.data.length;
    }

    public int compareToIgnoreCase(String other) {
        if (this == other) {
            return 0;
        }

        int last = Math.min(this.data.length, other.data.length);
        for (int i = 0; i < last; i++) {
            char currentLeft = Character.toLowerCase(this.charAt(i));
            char currentRight = Character.toLowerCase(other.charAt(i));
            if (currentLeft - currentRight != 0) {
                return currentLeft - currentRight;
            }
        }

        return last - other.data.length;
    }

    public boolean startsWith(String start) {
        return this.startsWith(start, 0);
    }

    public boolean startsWith(String start, int offset) {
        // TODO: This could be made more direct
        if (this.data.length >= start.data.length + offset) {
            String startArea = this.substring(offset, start.data.length);
            return startArea.equals(start);
        }

        return false;
    }

    public boolean endsWith(String end) {
        if (this.data.length >= end.data.length) {
            String endArea = this.substring(this.data.length - end.data.length);
            return endArea.equals(end);
        }

        return false;
    }

    public int indexOf(int chr) {
        for (int i = 0; i < this.data.length; i++) {
            if (this.data[i] == chr) {
                return i;
            }
        }

        return -1;
    }

    public int indexOf(int chr, int start) {
        for (int i = start; i < this.data.length; i++) {
            if (this.data[i] == chr) {
                return i;
            }
        }

        return -1;
    }

    public int indexOf(String other) {
        return this.indexOf(other, 0);
    }

    public int indexOf(String other, int start) {
        if (other.isEmpty()) {
            return start;
        }

        for (int i = start; i < this.data.length - other.data.length + 1; i++) {
            int k = 0;
            for (; k < other.data.length; k++) {
                if (this.data[i + k] != other.data[k]) {
                    break;
                }
            }

            if (k == other.data.length) {
                return i;
            }
        }

        return -1;
    }

    static int indexOf(char[] target, int targetLength, String other, int start) {
        if (other.isEmpty()) {
            return start;
        }

        for (int i = start; i < targetLength - other.data.length + 1; i++) {
            int k = 0;
            for (; k < other.data.length; k++) {
                if (target[i + k] != other.data[k]) {
                    break;
                }
            }

            if (k == other.data.length) {
                return i;
            }
        }

        return -1;
    }

    public int lastIndexOf(int chr)  {
        return this.lastIndexOf(chr, this.data.length - 1);
    }

    public int lastIndexOf(int chr, int last) {
        if (last >= this.data.length) {
            last = this.data.length - 1;
        }

        for (int i = last; i >= 0; i--) {
            if (this.data[i] == chr) {
                return i;
            }
        }

        return -1;
    }

    public int lastIndexOf(String other, int last) {
        if (other.isEmpty()) {
            return last;
        }

        int start = Math.min(this.data.length - other.data.length, last);

        for (int i = start; i >= 0; i--) {
            int k = 0;
            for (; k < other.data.length && i + k < this.data.length; k++) {
                if (this.data[i + k] != other.data[k]) {
                    break;
                }
            }

            if (k == other.data.length) {
                return i;
            }
        }

        return -1;
    }

    static int lastIndexOf(char[] target, int targetLength, String other, int last) {
        if (other.isEmpty()) {
            return last;
        }

        int start = Math.min(targetLength - other.data.length, last);

        for (int i = start; i >= 0; i--) {
            int k = 0;
            for (; k < other.data.length && i + k < targetLength; k++) {
                if (target[i + k] != other.data[k]) {
                    break;
                }
            }

            if (k == other.data.length) {
                return i;
            }
        }

        return -1;
    }

    public String substring(int start) {
        if (start < 0) {
            throw new StringIndexOutOfBoundsException(start);
        } else if (start > this.data.length) {
            throw new StringIndexOutOfBoundsException(start);
        }

        int len = this.data.length - start;
        if (start == 0) {
            return this;
        } else if (len == 0) {
            return "";
        } else {
            return new String(data, start, len);
        }
    }

    public String substring(int start, int end) {
        if (start < 0) {
            throw new StringIndexOutOfBoundsException(start);
        } else if (end > this.data.length) {
            throw new StringIndexOutOfBoundsException(end);
        } else if (end < start) {
            throw new StringIndexOutOfBoundsException(end - start);
        }

        int len = end - start;
        if (start == 0 && end == this.data.length) {
            // Strings are immutable so just use the same object
            return this;
        } else if (len == 0) {
            return "";
        } else {
            return new String(data, start, len);
        }
    }

    public CharSequence subSequence(int start, int end) {
        return this.substring(start, end);
    }

    public String concat(String other) {
        if (other.data.length == 0) {
            return this;
        }

        return this + other;
    }

    public String replace(char prev, char repl) {
        if (prev == repl) {
            return this;
        }

        // TODO: This does more work than it might need to?
        char[] res = new char[this.data.length];
        boolean found = false;
        for (int i = 0; i < this.data.length; i++) {
            if (this.data[i] == prev) {
                res[i] = repl;
                found = true;
            } else {
                res[i] = this.data[i];
            }
        }

        if (found) {
            return String.makeDirect(res);
        } else {
            return this;
        }
    }

    public boolean matches(String re) {
        return Pattern.matches(re, this);
    }

    public String replaceFirst(String re, String repl) {
        return Pattern.compile(re).matcher(this).replaceFirst(repl);
    }

    public String replaceAll(String re, String repl) {
        return Pattern.compile(re).matcher(this).replaceAll(repl);
    }

    public String[] split(String re) {
        return Pattern.compile(re).split(this, 0);
    }

    public String[] split(String re, int limit) {
        return Pattern.compile(re).split(this, limit);
    }

    public String trim() {
        int start = 0;
        for (; start < this.data.length; start++) {
            if (!Character.isWhitespace(this.data[start])) {
                break;
            }
        }

        int end = -1;
        for (int i = this.data.length - 1; i >= 0; i--) {
            if (end == -1 && !Character.isWhitespace(this.data[i])) {
                end = i + 1;
                break;
            }
        }

        if (start >= end) {
            return "";
        } else {
            return substring(start, end);
        }
    }



    public int hashCode() {
        if (hash == 0) {
            int output = 0;
            for (int i = 0; i < this.data.length; i++) {
                output = (output * 31) + this.data[i];
            }

            hash = output;
        }

        return hash;
    }

    public char[] toCharArray() {
        char output[] = new char[this.data.length];
        System.arraycopy(this.data, 0, output, 0, this.data.length);
        return output;
    }

    public String toString() {
        return this;
    }

    public static String valueOf(Object o) {
        if (o == null) {
            return "null";
        } else {
            return o.toString();
        }
    }

    public static String valueOf(char data[]) {
        return new String(data);
    }

    public static String valueOf(char data[], int offset, int length) {
        return new String(data, offset, length);
    }

    public static String valueOf(boolean b) {
        if (b) {
            return "true";
        } else {
            return "false";
        }
    }

    public static String valueOf(char chr) {
        return Character.toString(chr);
    }

    public static String valueOf(int val) {
        return Integer.toString(val);
    }

    public static String valueOf(long val) {
        return Long.toString(val);
    }

    public static String valueOf(float val) {
        return Float.toString(val);
    }

    public static String valueOf(double val) {
        return Double.toString(val);
    }

    public static String copyValueOf(char data[], int offset, int length) {
        return new String(data, offset, length);
    }

    public static String copyValueOf(char data[]) {
        return new String(data);
    }

    public static String format(String fmt, Object... args) {
        return new Formatter().format(fmt, args).toString();
    }

    public static String format(Locale locale, String fmt, Object... args) {
        return new Formatter(locale).format(fmt, args).toString();
    }

    public String intern() {
        // This probably just searches the heap? ehh
        throw new UnsupportedOperationException("TODO: implement intern method");
    }
    
    // TODO: regionMatches
    // TODO: StringBuffer/StringBuilder constructors?
    // TODO: implement deprecrated string constructors?
}