package rho;

import java.util.NoSuchElementException;
import java.util.Enumeration;

// Note: Some internal code may assume
//  that this has no fields and so does not bother running the constructor
public class EmptyEnumeration<E> implements Enumeration<E> {
    EmptyEnumeration () {}

    public boolean hasMoreElements() {
        return false;
    }

    public E nextElement() {
        throw new NoSuchElementException("Empty Enumeration");
    }
}