// The save rule of an HBM 1.7.10 text field (GUIRBMKControlAuto.mouseClicked:108-118) over every string of up to four
// characters of an alphabet: commons-lang3 3.3.2 `NumberUtils.isNumber`, then `Double.parseDouble` (which may throw
// where isNumber passes), `MathHelper.clamp_double` to [0, clamp] and the `(int)` cast. One line per string:
// <string as \\uXXXX escapes><TAB><isNumber 0/1><TAB><bits of parseDouble, or EXC><TAB><int for clamp 100><TAB><int for 9999>
// Run: javac -cp commons-lang3-3.3.2.jar NumberOracle.java && java -cp commons-lang3-3.3.2.jar:. NumberOracle > out.tsv
import org.apache.commons.lang3.math.NumberUtils;

public class NumberOracle {
    static final String ALPHABET = "018.-+eExXdDfFlLa ";

    static double clamp(double v, double min, double max) {
        return v < min ? min : (v > max ? max : v); // MathHelper.clamp_double
    }

    static String escape(String s) {
        StringBuilder b = new StringBuilder();
        for (char c : s.toCharArray()) b.append(String.format("\\u%04x", (int) c));
        return b.toString();
    }

    static void emit(String s, StringBuilder out) {
        boolean number = NumberUtils.isNumber(s);
        String bits, c100, c9999;
        try {
            double d = Double.parseDouble(s);
            bits = Long.toString(Double.doubleToRawLongBits(d));
            c100 = Integer.toString((int) clamp(d, 0, 100));
            c9999 = Integer.toString((int) clamp(d, 0, 9999));
        } catch (NumberFormatException e) {
            bits = "EXC"; c100 = "EXC"; c9999 = "EXC";
        }
        out.append(escape(s)).append('\t').append(number ? 1 : 0).append('\t').append(bits).append('\t').append(c100).append('\t').append(c9999).append('\n');
    }

    public static void main(String[] args) {
        StringBuilder out = new StringBuilder();
        emit("", out);
        int n = ALPHABET.length();
        for (int len = 1; len <= 4; len++) {
            int[] idx = new int[len];
            while (true) {
                char[] cs = new char[len];
                for (int i = 0; i < len; i++) cs[i] = ALPHABET.charAt(idx[i]);
                emit(new String(cs), out);
                int k = len - 1;
                while (k >= 0 && ++idx[k] == n) { idx[k] = 0; k--; }
                if (k < 0) break;
            }
        }
        System.out.print(out);
    }
}
