import java.util.ArrayList;
import java.util.List;

public class InvDyn {
	public static void main(String[] args) {
	    ArrayList<String> colors = new ArrayList<String>();
	    colors.add("Red");
	    colors.add("Green");
	    colors.add("Blue");
		long lengthyColors = colors
			.stream().filter(c -> c.length() > 3).count();
	}
}
