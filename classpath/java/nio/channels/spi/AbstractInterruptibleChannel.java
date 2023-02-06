package java.nio.channels.spi;

import java.io.IOException;

import java.nio.channels.Channel;
import java.nio.channels.InterruptibleChannel;
import java.nio.channels.AsynchronousCloseException;

public abstract class AbstractInterruptibleChannel implements Channel, InterruptibleChannel {
    private volatile boolean open = true;
    
    protected AbstractInterruptibleChannel() {}

    public boolean isOpen() {
        return open;
    }

    protected void begin() {
        // TODO: create interrupter?
    }

    protected void end(boolean completed) throws AsynchronousCloseException {
        if (!completed && !open) {
            throw new AsynchronousCloseException();
        }
    }

    public final void close() throws IOException {
        // TODO: Synchronization
        if (!open) {
            return;
        }

        open = false;
        implCloseChannel();
    }

    protected abstract void implCloseChannel() throws IOException;
}