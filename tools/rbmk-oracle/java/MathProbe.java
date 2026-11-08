import java.util.*;

/** Records HotSpot's Math.log10 / Math.pow(Math.E, y) / Math.sin / Math.cos bits for
 *  jmath parity. Run: java tools/rbmk-oracle/java/MathProbe.java > tools/rbmk-oracle/fixtures/jmath.txt
 *  Each line: kind argbits resultbits. Deterministic (fixed seed). */
public class MathProbe {
    public static void main(String[] a) {
        Random r = new Random(20261108L);
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 1500; i++) {
            double x = r.nextDouble() * 3, y = (r.nextDouble() - 0.5) * 60, s = r.nextDouble() * Math.PI, c = (r.nextDouble() - 0.5) * 400;
            if (i % 5 == 0) x = Math.pow(10, r.nextInt(20) - 8);
            sb.append("log10 ").append(Double.doubleToLongBits(x)).append(' ').append(Double.doubleToLongBits(Math.log10(x))).append('\n');
            sb.append("powe ").append(Double.doubleToLongBits(y)).append(' ').append(Double.doubleToLongBits(Math.pow(Math.E, y))).append('\n');
            sb.append("sin ").append(Double.doubleToLongBits(s)).append(' ').append(Double.doubleToLongBits(Math.sin(s))).append('\n');
            sb.append("sin ").append(Double.doubleToLongBits(c)).append(' ').append(Double.doubleToLongBits(Math.sin(c))).append('\n');
            sb.append("cos ").append(Double.doubleToLongBits(c)).append(' ').append(Double.doubleToLongBits(Math.cos(c))).append('\n');
        }
        // the reactor's own call shapes: surge pow(1 - level, 15), pow(x, 2), the depletion curves' sin arguments
        for (int i = 0; i < 1500; i++) {
            double level = i < 750 ? r.nextDouble() : Math.min(1.0, i * 0.00277 % 1.0 + r.nextInt(2) * 0.0);
            double x = 1.0 - level, e = r.nextDouble();
            sb.append("pow15 ").append(Double.doubleToLongBits(x)).append(' ').append(Double.doubleToLongBits(Math.pow(x, 15))).append('\n');
            sb.append("pow2 ").append(Double.doubleToLongBits(x)).append(' ').append(Double.doubleToLongBits(Math.pow(x, 2))).append('\n');
            for (double ang : new double[]{(e - 1) * (e - 1) * Math.PI, e * Math.PI, (e * Math.PI + Math.PI) / 2})
                sb.append("sindep ").append(Double.doubleToLongBits(ang)).append(" ").append(Double.doubleToLongBits(Math.sin(ang))).append('\n');
        }
        System.out.print(sb);
    }
}
