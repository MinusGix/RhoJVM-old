package sun.misc;

import java.nio.ByteBuffer;

import java.security.PrivilegedAction;

public final class Perf {
    private static Perf perf;

    static {
        perf = new Perf();
    }

    private Perf() {}

    public static class GetPerfAction implements PrivilegedAction<Perf> {
        public Perf run() {
            return getPerf();
        }
    }

    public static Perf getPerf() {
        return perf;
    }

    public ByteBuffer attach(int id, String mode) {
        throw new UnsupportedOperationException("attach(int, String)");
    }

    public ByteBuffer attach(String user, int id, String mode) {
        throw new UnsupportedOperationException("attach(String, int, String)");
    }

    public ByteBuffer createLong(String name, int variability, int units, long value) {
        throw new UnsupportedOperationException("createLong(String, int, int, long)");
    }

    public ByteBuffer createString(String name, int variability, int units, String value) {
        throw new UnsupportedOperationException("createString(String, int, int, String)");
    }

    public ByteBuffer createString(String name, int variability, int units, String value, int maxLength) {
        throw new UnsupportedOperationException("createString(String, int, int, String, int)");
    }

    public ByteBuffer createByteArray(String name, int variability, int units, byte[] value, int maxLength) {
        throw new UnsupportedOperationException("createByteArray(String, int, int, byte[], int)");
    }

    public long highResCounter() {
        return 0;
    }

    public long highResFrequency() {
        return 0;
    }
}