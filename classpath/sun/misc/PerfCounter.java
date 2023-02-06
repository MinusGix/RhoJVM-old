package sun.misc;

public class PerfCounter {
    static final PerfCounter zip = new PerfCounter("sun.zip.zipFiles");
    static final PerfCounter zipTime = new PerfCounter("sun.zip.zipFile.openTime");

    private String name;
    private long value;

    private PerfCounter(String name) {
        this.name = name;
    }

    public synchronized long get() {
        return value;
    }

    public synchronized void set(long val) {
        value = val;
    }

    public synchronized void add(long val) {
        value += val;
    }

    public void increment() {
        add(1);
    }

    public void addTime(long time) {
        value += time;
    }

    public void addElapsedTimeFrom(long startTime) {
        value += System.nanoTime() - startTime;
    }

    @Override
    public String toString() {
        return name + " = " + value;
    }

    // Optimally, we shouldn't need any of these.
    // However, we may need to 'implement' them so that general java programs can run in our 
    // jvm..
    public static PerfCounter getFindClasses() {
        throw new UnsupportedOperationException("getFindClasses()");
    }

    public static PerfCounter getFindResources() {
        throw new UnsupportedOperationException("getFindResources()");
    }

    public static PerfCounter getReadClassBytesTime() {
        throw new UnsupportedOperationException("getReadClassBytesTime()");
    }

    public static PerfCounter getParentDelegationTime() {
        throw new UnsupportedOperationException("getParentDelegationTime()");
    }

    public static PerfCounter getZipFileCount() {
        return zip;
    }

    public static PerfCounter getZipFileOpenTime() {
        return zipTime;
    }

    public static PerfCounter getD3DAvailable() {
        throw new UnsupportedOperationException("getD3DAvailable()");
    }
}