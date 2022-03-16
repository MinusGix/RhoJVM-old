package java.lang;

import java.util.Map;

public class Thread implements Runnable {
    public final static int MIN_PRIORITY = 1;
    public final static int NORM_PRIORITY = 5;
    public final static int MAX_PRIORITY = 10;

    public enum State {
        NEW,
        RUNNABLE,
        BLOCKED,
        WAITING,
        TIMED_WAITING,
        TERMINATED,
    }

    public interface UncaughtExceptionHandler {
        void uncaughtException(Thread thread, Throwable err);
    }

    public Thread(Runnable code) {
        throw new UnsupportedOperationException();
    }

    public Thread(String name) {
        this(null, null, name);
    }
    
    public Thread(Runnable code, String name) {
        throw new UnsupportedOperationException();
    }

    public Thread(ThreadGroup group, Runnable code) {
        throw new UnsupportedOperationException();
    }

    public Thread(ThreadGroup group, String name) {
        throw new UnsupportedOperationException();
    }

    public Thread(ThreadGroup group, Runnable code, String name) {
        throw new UnsupportedOperationException();
    }

    public Thread(ThreadGroup group, Runnable code, String name, long stackSize) {
        throw new UnsupportedOperationException();
    }

    public static native Thread currentThread();

    public static void yield() {}

    public static void sleep(long milliseconds) throws InterruptedException {
        // TODO
    }

    public static void sleep(long milliseconds, int nanoseconds) throws InterruptedException {
        if (milliseconds < 0) {
            throw new IllegalArgumentException("Negative time");
        }

        if (nanoseconds < 0 || nanoseconds > 999999) {
            throw new IllegalArgumentException("Nanoseconds outside of allowed range");
        }

        // TODO: Include nanoseconds

        Thread.sleep(milliseconds);
    }

    public synchronized void start() {
        throw new UnsupportedOperationException();
    }

    public void run() {
        throw new UnsupportedOperationException();
    }

    public final void stop() {
        throw new UnsupportedOperationException();
    }

    public final synchronized void stop(Throwable o) {
        throw new UnsupportedOperationException();
    }

    public void interrupt() {
        throw new UnsupportedOperationException();
    }

    public static boolean interrupted() {
        throw new UnsupportedOperationException();
    }

    public boolean isInterrupted() {
        throw new UnsupportedOperationException();
    }

    public final boolean isAlive() {
        throw new UnsupportedOperationException();
    }

    public final void suspend() {
        throw new UnsupportedOperationException();
    }

    public final void resume() {
        throw new UnsupportedOperationException();
    }

    public void destroy() {
        throw new UnsupportedOperationException();
    }

    public final void join() throws InterruptedException {
        this.join(0);
    }

    public final synchronized void join(long milliseconds) throws InterruptedException {
        throw new UnsupportedOperationException();
    }

    public final synchronized void join(long milliseconds, int nanoseconds) {
        throw new UnsupportedOperationException();
    }


    public final void setPriority(int priority) {
        throw new UnsupportedOperationException();
    }

    public final int getPriority() {
        throw new UnsupportedOperationException();
    }

    public final synchronized void setName(String name) {
        throw new UnsupportedOperationException();
    }

    public final String getName() {
        throw new UnsupportedOperationException();
    }

    public long getId() {
        throw new UnsupportedOperationException();
    }

    public State getState() {
        throw new UnsupportedOperationException();
    }

    public final ThreadGroup getThreadGroup() {
        throw new UnsupportedOperationException();
    }

    public static int activeCount() {
        throw new UnsupportedOperationException();
    }

    public static int enumerate(Thread output[]) {
        throw new UnsupportedOperationException();
    }    

    public final void setDaemon(boolean isDaemon) {
        throw new UnsupportedOperationException();
    }

    public final boolean isDaemon() {
        throw new UnsupportedOperationException();
    }

    public final void checkAccess() {
        throw new UnsupportedOperationException();
    }

    public ClassLoader getContextClassLoader() {
        throw new UnsupportedOperationException();
    }

    public void setContextClassLoader(ClassLoader loader) {
        throw new UnsupportedOperationException();
    }

    public static boolean holdsLock(Object o) {
        throw new UnsupportedOperationException();
    }

    public static void setDefaultUncaughtExceptionHandler(UncaughtExceptionHandler handler) {
        // Silently ignore
        // FIXME: Don't ignore this
    }

    public static UncaughtExceptionHandler getDefaultUncaughtExceptionHandler() {
        throw new UnsupportedOperationException();
    }

    public void setUncaughtExceptionHandler(UncaughtExceptionHandler handler) {
        // Silently ignore
        // FIXME: Don't ignore this
    }

    public UncaughtExceptionHandler getUncaughtExceptionHandler() {
        throw new UnsupportedOperationException();
    }

    public int countStackFrames() {
        throw new UnsupportedOperationException();
    }

    public static void dumpStack() {
        throw new UnsupportedOperationException();
    }

    public StackTraceElement[] getStackTrace() {
        throw new UnsupportedOperationException();
    }

    public static Map<Thread, StackTraceElement[]> getAllStackTraces() {
        throw new UnsupportedOperationException();
    }

    protected Object clone() throws CloneNotSupportedException {
        throw new CloneNotSupportedException();
    }

    public String toString() {
        return "Thread[" + this.getName() + ", " + this.getPriority() + "]";
    }
}