package rho.util;

import java.lang.Enum;

import java.util.EnumSet;
import java.util.Collection;
import java.util.Iterator;
import java.util.ArrayList;

public class EnumSetInst<E extends Enum<E>> extends EnumSet<E> {
    // TODO: This could be way more efficient since java enums are just ints and so we could store them like a single int for most enums!
    private ArrayList<E> elements = new ArrayList<E>();

    public EnumSetInst(Class<E> elemTy) {
        super(elemTy);
    }

    public static <E extends Enum<E>> EnumSet<E> allOfImpl(Class<E> elemTy) {
        EnumSet<E> set = new EnumSetInst<E>(elemTy);
        for (E e : elemTy.getEnumConstants()) {
            set.add(e);
        }
        return set;
    }

    public boolean contains(Object o) {
        return elements.contains(o);
    }

    public boolean containsAll(Collection<?> c) {
        return elements.containsAll(c);
    }

    public boolean add(E e) {
        if (elements.contains(e)) {
            return false;
        } else {
            elements.add(e);
            return true;
        }
    }

    public boolean remove(Object o) {
        return elements.remove(o);
    }

    public void clear() {
        elements.clear();
    }

    public int size() {
        return elements.size();
    }

    public boolean isEmpty() {
        return elements.isEmpty();
    }

    public Iterator<E> iterator() {
        return elements.iterator();
    }
}