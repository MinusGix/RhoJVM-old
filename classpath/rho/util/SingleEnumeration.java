package rho;

import java.util.NoSuchElementException;
import java.util.Enumeration;

// An enumeration with a single value
public class SingleEnumeration<E> implements Enumeration<E> {
    private E value;
    private boolean consumed = false;

    SingleEnumeration (E value) {
        this.value = value;
    }

    public boolean hasMoreElements() {
        return !this.consumed;
    }

    public E nextElement() {
        if (this.consumed) {
            throw new NoSuchElementException("Single Enumeration");
        } else {
            this.consumed = true;
            return this.value;
        }
    }
}