package java.lang;

import java.io.IOException;
import java.io.File;
import java.io.OutputStream;

public class Runtime extends Object {
    private static final Runtime instance = new Runtime();

    public static Runtime getRuntime() {
        return Runtime.instance;
    }

    // Exiting

    public static void runFinalizersOnExit(boolean value) {
        throw new UnsupportedOperationException();
    }

    public void addShutdownHook(Thread hook) {
        throw new UnsupportedOperationException();
    }

    public boolean removeShutdownHook(Thread hook) {
        throw new UnsupportedOperationException();
    }

    public void exit(int status) {
        throw new UnsupportedOperationException();
    }

    public void halt(int status) {
        throw new UnsupportedOperationException();
    }

    // Executing

    public Process exec(String cmd) throws IOException {
        throw new UnsupportedOperationException();
    }

    public Process exec(String cmd, String[] env) throws IOException {
        throw new UnsupportedOperationException();
    }

    public Process exec(String cmd, String[] env, File directory) throws IOException {
        throw new UnsupportedOperationException();
    }

    public Process exec(String[] cmds) throws IOException {
        throw new UnsupportedOperationException();
    }

    public Process exec(String[] cmds, String[] env) throws IOException {
        throw new UnsupportedOperationException();
    }

    public Process exec(String[] cmds, String[] env, File directory) throws IOException {
        throw new UnsupportedOperationException();
    }

    // System/Processor info

    public native int availableProcessors();

    public native long freeMemory();

    public native long totalMemory();

    public native long maxMemory();

    public void gc() {
        // TODO
    }

    public void runFinalization() {
        throw new UnsupportedOperationException();
    }

    // Tracing

    public void traceInstructions(boolean enable) {
        throw new UnsupportedOperationException();
    }

    public void traceMethodCalls(boolean enable) {
        throw new UnsupportedOperationException();
    }

    // Library Loading

    public void load(String libraryPath) {
        throw new UnsupportedOperationException();
    }

    public void loadLibrary(String libraryName) {
        throw new UnsupportedOperationException();
    }

    //

    public OutputStream getLocalizedOutputStream(OutputStream out) {
        throw new UnsupportedOperationException();
    }
}