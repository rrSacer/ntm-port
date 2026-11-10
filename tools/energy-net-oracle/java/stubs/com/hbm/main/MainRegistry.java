package com.hbm.main;
/** Stub: only the members the sliced REDD client branch and ItemBatteryPack ctor name. */
public class MainRegistry {
	public static Object controlTab = null;
	public static Proxy proxy = new Proxy();
	public static class Proxy { public Player me() { return new Player(); } public com.hbm.sound.AudioWrapper getLoopedSound(String s, float x, float y, float z, float v, float r, float p, int k) { return null; } }
	public static class Player { public double getDistanceSq(double x, double y, double z) { return 0; } }
}
