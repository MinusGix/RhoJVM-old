package java.lang;

import rho.util.Log;

public class UnsupportedOperationException extends RuntimeException {
    public UnsupportedOperationException() {
        this.checkAbort();
    }

    private native void checkAbort();

    public UnsupportedOperationException(String message) {
        super(message);
        Log.warn("UnsupportedOperationException created: " + message);
        this.checkAbort();
    }

    public UnsupportedOperationException(String message, Throwable cause) {
        super(message, cause);
        this.checkAbort();
    }

    public UnsupportedOperationException(Throwable cause) {
        super(cause);
        this.checkAbort();
    }
}