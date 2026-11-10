package com.hbm.uninos;
import com.hbm.util.fauxpointtwelve.BlockPos;
import com.hbm.util.fauxpointtwelve.DirPos;
public class GenNode<N extends NodeNet> {
	public BlockPos[] positions;
	public DirPos[] connections;
	public N net;
	public boolean expired = false;
	public GenNode(BlockPos... positions) { this.positions = positions; }
	public GenNode<N> setConnections(DirPos... connections) { this.connections = connections; return this; }
	public void setNet(N net) { this.net = net; }
	public boolean hasValidNet() { return this.net != null && this.net.isValid(); }
}
