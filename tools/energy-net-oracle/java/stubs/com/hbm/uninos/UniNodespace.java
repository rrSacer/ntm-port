package com.hbm.uninos;
import api.hbm.energymk2.Nodespace.PowerNode;
import java.util.ArrayList;
import java.util.List;
/** Stub: one shared test net; getNode/createNode keep a single node per tile so the real update() membership code runs. */
public class UniNodespace {
	public static List<NodeNet> activeNodeNets = new ArrayList<>();
	public static api.hbm.energymk2.PowerNetMK2 testNet;
	public static java.util.Map<Object, GenNode> nodes = new java.util.HashMap<>();
	public static GenNode getNode(Object world, int x, int y, int z, Object provider) { return nodes.get(x + "," + y + "," + z); }
	public static void createNode(Object world, GenNode node) {
		node.net = testNet;
		nodes.put(node.positions[0].getX() + "," + node.positions[0].getY() + "," + node.positions[0].getZ(), node);
	}
	public static void destroyNode(Object world, int x, int y, int z, Object provider) { nodes.remove(x + "," + y + "," + z); }
}
