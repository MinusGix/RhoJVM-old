package java.lang;

import java.io.Serializable;
import java.util.Arrays;

// TODO: There are a bunch of optimizations that could be applied to this
public final class StringBuilder implements Serializable, CharSequence {
    private char[] data;
    private int usedLength;

    public StringBuilder() {
        this(16);
    }

    public StringBuilder(int capacity) {
        data = new char[capacity];
    }

    public StringBuilder(String source) {
        this(16);
        this.append(source);
    }

    public char charAt(int index) {
        if (index >= this.usedLength || index < 0) {
            throw new StringIndexOutOfBoundsException(index);
        }

        return this.data[index];
    }

    public void setCharAt(int index, char chr) {
        if (index >= this.usedLength || index < 0) {
            throw new StringIndexOutOfBoundsException(index);
        }

        this.data[index] = chr;
    }

    // TODO: codePointAt
    // TODO: codePointBefore
    // TODO: codePointCount
    // TODO: offsetByCodePoints
    // TODO: appendCodePoint

    public void ensureCapacity(int capacity) {
        if (capacity > 0 && capacity > this.data.length) {
            expandTo(capacity);
        }
    }

    private void expandTo(int capacity) {
        // TODO: Should we do a smarter method than just growing a bit larger? We could do like 
        // Rust's vecs do and double?
        this.data = Arrays.copyOf(this.data, capacity);
    }

    public void trimToSize() {
        if (this.usedLength < this.data.length) {
            this.data = Arrays.copyOf(this.data, this.usedLength);
        }
    }

    public int length() {
        return this.usedLength;
    }

    public void setLength(int length) {
        if (length < 0) {
            throw new StringIndexOutOfBoundsException(length);
        }

        this.ensureCapacity(length);

        if (this.usedLength < length) {
            // Fill it with empty characters
            Arrays.fill(this.data, this.usedLength, length, '\0');
        }

        this.usedLength = length;
    }

    public void getChars(int sourceBegin, int sourceEnd, char[] destination, int destinationBegin) {
        if (sourceBegin < 0) {
            throw new StringIndexOutOfBoundsException(sourceBegin);
        } else if (sourceEnd > this.usedLength || sourceEnd < 0) {
            throw new StringIndexOutOfBoundsException(sourceEnd);
        } else if (sourceBegin > sourceEnd) {
            throw new StringIndexOutOfBoundsException("Source begin was after source end");
        }

        System.arraycopy(this.data, sourceBegin, destination, destinationBegin, sourceEnd - sourceBegin);
    }

    public StringBuilder append(String source) {
        if (source == null) {
            return this.append("null");
        }

        if (!source.isEmpty()) {
            int length = source.length();
            this.ensureCapacity(this.usedLength + length);
            for (int i = 0; i < length; i++) {
                this.data[i + this.usedLength] = source.charAt(i);
            }
            this.usedLength += length;
        }

        return this;
    }

    public StringBuilder append(StringBuffer source) {
        return this.append(source.toString());
    }

    public StringBuilder append(char[] source) {
        ensureCapacity(this.usedLength + source.length);
        System.arraycopy(source, 0, this.data, this.usedLength, source.length);
        this.usedLength += source.length;
        
        return this;
    }

    public StringBuilder append(char source[], int start, int count) {
        if (count < 0) {
            throw new IndexOutOfBoundsException("count");
        } else if (start < 0) {
            throw new IndexOutOfBoundsException("start");
        } else if (start + count > source.length) {
            throw new IndexOutOfBoundsException();
        }

        System.arraycopy(source, start, this.data, this.usedLength, count);
        this.usedLength += count;

        return this;
    }

    public StringBuilder append(CharSequence source) {
        return this.append(source, 0, source.length());
    }

    public StringBuilder append(CharSequence source, int start, int end) {
        if (source == null) {
            return this.append("null");
        }

        if (start < 0) {
            throw new IndexOutOfBoundsException("start");
        } else if (start > end) {
            throw new IndexOutOfBoundsException("start is past end");
        } else if (end > source.length()) {
            throw new IndexOutOfBoundsException("end is pass source's length");
        }

        int count = end - start;

        ensureCapacity(this.usedLength + count);

        for (int i = 0; i < count; i++) {
            this.data[i + this.usedLength] = source.charAt(i + start);
        }

        this.usedLength += count;
        return this;
    }

    public StringBuilder append(boolean source) {
        if (source) {
            return this.append("true");
        } else {
            return this.append("false");
        }
    }

    public StringBuilder append(char source) {
        ensureCapacity(this.usedLength + 1);
        this.data[this.usedLength] = source;
        this.usedLength++;
        return this;
    }

    public StringBuilder append(int source) {
        return this.append(String.valueOf(source));
    }

    public StringBuilder append(long source) {
        return this.append(String.valueOf(source));
    }

    public StringBuilder append(float source) {
        return this.append(String.valueOf(source));
    }

    public StringBuilder append(double source) {
        return this.append(String.valueOf(source));
    }

    public StringBuilder append(Object source) {
        return this.append(String.valueOf(source));
    }

    // TODO:
    // public StringBuilder delete(int start, int end) {}
    // public StringBuilder deleteCharAt(int index) {}
    
    // public StringBuilder replace(int start, int end, String source) {}

    // TODO: Substring
    
    public CharSequence subSequence(int start, int end) {
        throw new UnsupportedOperationException("TODO: implement subSequence in terms of substring");
    }

    // TODO: Insert

    // TODO: Reverse

    public int indexOf(String source) {
        return String.indexOf(this.data, this.usedLength, source, 0);
    }

    public int indexOf(String source, int start) {
        return String.indexOf(this.data, this.usedLength, source, start);
    }

    public int lastIndexOf(String source) {
        return String.lastIndexOf(this.data, this.usedLength, source, 0);
    }

    public int lastIndexOf(String source, int start) {
        return String.lastIndexOf(this.data, this.usedLength, source, start);
    }

    public String toString() {
        return new String(this.data, 0, this.usedLength);
    }
}