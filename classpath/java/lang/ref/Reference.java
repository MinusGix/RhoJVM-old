package java.lang.ref;

// Currently not handled specially
public abstract class Reference<T> {
    private T value;
    ReferenceQueue<? super T> queue;

    Reference(T value) {
        this.value = value;
    }

    Reference(T value, ReferenceQueue<? super T> queue) {
        this.value = value;
        if (queue == null) {
            this.queue = ReferenceQueue.NULL;
        } else {
            this.queue = queue;
        }
    }

    public T get() {
        return this.value;
    }

    public void clear() {
        this.value = null;
    }

    public boolean isEnqueued() {
        return this.queue == ReferenceQueue.ENQUEUED;
    }

    public boolean enqueue() {
        return this.queue.enqueue(this);
    }
}