package api.hbm.energymk2;
import com.hbm.uninos.GenNode;
import com.hbm.util.fauxpointtwelve.BlockPos;
import com.hbm.util.fauxpointtwelve.DirPos;
import java.util.ArrayList;
import java.util.List;
/** Stub: PowerNode only records positions/connections (PowerNode ctor/setConnections/addConnection). */
public class Nodespace {
	public static final Object THE_POWER_PROVIDER = new Object();
	public static class PowerNode extends GenNode<PowerNetMK2> {
		public List<DirPos> extra = new ArrayList<>();
		public PowerNode(BlockPos... positions) { super(positions); }
		@Override public PowerNode setConnections(DirPos... connections) { super.setConnections(connections); return this; }
		public void addConnection(DirPos pos) { extra.add(pos); }
	}
}
