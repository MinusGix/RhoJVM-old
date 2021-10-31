import java.lang.Throwable;
public class HelloWorld extends RuntimeException {
    public static void main (String[] args) {
        try {
            System.out.println("Hello World!");
        } catch (RuntimeException v) {
            System.out.println(v);
        }
    }

    public String getMessage() {
        return "Hello";
    }
}