package java.lang;

public class Object {
    public final native Class<?> getClass();

    public native int hashCode();

    public boolean equals(Object other) {
        return this == other;
    }

    protected native Object clone() throws CloneNotSupportedException;

    public String toString() {
        return this.getClass().getName() + "@" + Integer.toHexString(this.hashCode());
    }

    public final native void notify();

    public final native void notifyAll();

    public final native void wait(long timeout) throws InterruptedException;

    public final void wait(long timeout, int nanoseconds) throws InterruptedException {
        if (timeout < 0) {
            throw new IllegalArgumentException("negative timeout");
        } else if (nanoseconds > 999999 || nanoseconds < 0) {
            throw new IllegalArgumentException("invald number of nanoseconds");
        }

        throw new UnsupportedOperationException("TODO: Implement");
    }

    public final void wait() throws InterruptedException {
        this.wait(0);
    }

    protected void finalize() throws Throwable {}
}