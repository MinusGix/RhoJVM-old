package java.util;

import java.io.Serializable;
import rho.util.EnumSetInst;

public abstract class EnumSet<E extends Enum<E>> extends AbstractSet<E> implements Cloneable, Serializable {
    /// The element type we store
    private final Class<E> elemTy;

    protected EnumSet(Class<E> elemTy) {
        if (elemTy == null) {
            throw new NullPointerException("EnumSet's class cannot be null");
        }

        this.elemTy = elemTy;
    }

    public static <E extends Enum<E>> EnumSet<E> allOf(Class<E> elemTy) {
        return EnumSetInst.allOfImpl(elemTy);
    }

    public static <E extends Enum<E>> EnumSet<E> complementOf(EnumSet<E> other) {
        throw new UnsupportedOperationException("TODO: Implement this");
    }

    public static <E extends Enum<E>> EnumSet<E> copyOf(EnumSet<E> other) {
        return other.clone();
    }

    public static <E extends Enum<E>> EnumSet<E> copyOf(Collection<E> other) {
        if (other instanceof EnumSet) {
            return copyOf((EnumSet<E>) other);
        }

        if (other.isEmpty()) {
            throw new IllegalArgumentException("Collection is empty");
        }

        Iterator<E> iter = other.iterator();
        E first = iter.next();
        EnumSet<E> set = new EnumSetInst<E>(first.getDeclaringClass());
        set.add(first);
        while (iter.hasNext()) {
            set.add(iter.next());
        }
        return set;
    }

    public static <E extends Enum<E>> EnumSet<E> noneOf(Class<E> elemTy) {
        return new EnumSetInst<E>(elemTy);
    }

    public static <E extends Enum<E>> EnumSet<E> of(E elem) {
        EnumSet<E> set = new EnumSetInst<E>(elem.getDeclaringClass());
        // use our `add` method from AbstractSet
        set.add(elem);
        return set;
    }

    public static <E extends Enum<E>> EnumSet<E> of(E elem1, E elem2) {
        EnumSet<E> set = new EnumSetInst<E>(elem1.getDeclaringClass());
        set.add(elem1);
        set.add(elem2);
        return set;
    }

    public static <E extends Enum<E>> EnumSet<E> of(E elem1, E elem2, E elem3) {
        EnumSet<E> set = new EnumSetInst<E>(elem1.getDeclaringClass());
        set.add(elem1);
        set.add(elem2);
        set.add(elem3);
        return set;
    }

    public static <E extends Enum<E>> EnumSet<E> of(E elem1, E elem2, E elem3, E elem4) {
        EnumSet<E> set = new EnumSetInst<E>(elem1.getDeclaringClass());
        set.add(elem1);
        set.add(elem2);
        set.add(elem3);
        set.add(elem4);
        return set;
    }

    public static <E extends Enum<E>> EnumSet<E> of(E elem1, E elem2, E elem3, E elem4, E elem5) {
        EnumSet<E> set = new EnumSetInst<E>(elem1.getDeclaringClass());
        set.add(elem1);
        set.add(elem2);
        set.add(elem3);
        set.add(elem4);
        set.add(elem5);
        return set;
    }

    public static <E extends Enum<E>> EnumSet<E> of(E elem1, E... rest) {
        EnumSet<E> set = new EnumSetInst<E>(elem1.getDeclaringClass());
        set.add(elem1);
        for (E elem : rest) {
            set.add(elem);
        }
        return set;
    }

    public static <E extends Enum<E>> EnumSet<E> range(E from, E to) {
        if (from.compareTo(to) > 0) {
            throw new IllegalArgumentException("from must be <= to");
        }

        throw new UnsupportedOperationException("TODO: Implement this");

        // EnumSet<E> set = new EnumSet(from.getDeclaringClass());
        // TODO: ???    
        // return set;
    }

    public EnumSet<E> clone() {
        try {
            return (EnumSet<E>) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}