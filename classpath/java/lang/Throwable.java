package java.lang;

import java.io.PrintStream;
import java.io.Serializable;

public class Throwable implements Serializable {
    private String message;
    private Throwable cause;
    private StackTraceElement[] trace;

    public Throwable() {}

    public Throwable(String message) {
        this.message = message;
    }

    public Throwable(Throwable cause) {
        this.cause = cause;
    }

    public Throwable(String message, Throwable cause) {
        this.message = message;
        this.cause = cause;
    }

    public String getMessage() {
        return this.message;
    }

    public Throwable getCause() {
        return this.cause;
    }

    public String getLocalizedMessage() {
        return this.message;
    }

    public Throwable initCause(Throwable cause) {
        if (cause == this) {
            throw new IllegalArgumentException("cause");
        } else if (this.cause != null) {
            throw new IllegalStateException("Can't change the cause twice");
        } else {
            this.cause = cause;
            return this;
        }
    }

    public String toString() {
        String output = this.getClass().getName();
        if (this.message != null) {
            output += ": " + message;
        }
        return output;
    }

    public StackTraceElement[] getStackTrace() {
        // TODO: Are we supposed to guard against modifications?
        return this.trace;
    }

    public void setStackTrace(StackTraceElement[] trace) {
        this.trace = trace;
    }

    public Throwable fillInStackTrace() {
        throw new UnsupportedOperationException("TODO");
    }

    public void printStackTrace() {
        throw new UnsupportedOperationException("TODO");
    }

    public void printStackTrace(PrintStream output) {
        throw new UnsupportedOperationException("TODO");
    }

    public void addSuppressed(Throwable exc) {}

    public final Throwable[] getSuppresed() {
        return new Throwable[0];
    }
}