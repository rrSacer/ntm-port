"""Real keyboard events for native acceptance scripts (X11 XTest, Xvfb).

`horizon-ctl key` and `type` do not go through the window's keyboard event,
and a mod screen's keys do. Use this when a script must prove that path:

    keys = XInput(os.environ['DISPLAY'])
    keys.tap('Return'); keys.text('placa'); keys.tap('Escape')

Keysym names are X11's (`Return`, `Escape`, `Prior`, `Next`, `Home`, `End`).
"""
import ctypes as C
import time


class XInput:
    def __init__(self, display):
        self.x = C.CDLL('libX11.so.6')
        self.xt = C.CDLL('libXtst.so.6')
        x, xt = self.x, self.xt
        x.XOpenDisplay.argtypes = [C.c_char_p]
        x.XOpenDisplay.restype = C.c_void_p
        x.XDefaultRootWindow.argtypes = [C.c_void_p]
        x.XDefaultRootWindow.restype = C.c_ulong
        x.XQueryTree.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_ulong),
                                 C.POINTER(C.POINTER(C.c_ulong)), C.POINTER(C.c_uint)]
        x.XSetInputFocus.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong]
        x.XFlush.argtypes = [C.c_void_p]
        x.XFree.argtypes = [C.c_void_p]
        x.XStringToKeysym.argtypes = [C.c_char_p]
        x.XStringToKeysym.restype = C.c_ulong
        x.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]
        x.XKeysymToKeycode.restype = C.c_uint
        xt.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
        xt.XTestFakeMotionEvent.argtypes = [C.c_void_p, C.c_int, C.c_int, C.c_int, C.c_ulong]
        self.display = x.XOpenDisplay(display.encode())
        assert self.display, f'cannot open {display}'
        self.focus()

    def focus(self):
        """Focus the newest top-level window (the game's)."""
        x = self.x
        root, parent = C.c_ulong(), C.c_ulong()
        children, count = C.POINTER(C.c_ulong)(), C.c_uint()
        x.XQueryTree(self.display, x.XDefaultRootWindow(self.display), C.byref(root),
                     C.byref(parent), C.byref(children), C.byref(count))
        assert count.value, 'no window to focus'
        x.XSetInputFocus(self.display, children[count.value - 1], 2, 0)
        x.XFree(children)
        x.XFlush(self.display)
        time.sleep(.2)

    def edge(self, name, down):
        code = self.x.XKeysymToKeycode(self.display, self.x.XStringToKeysym(name.encode()))
        assert code, f'no keycode for {name}'
        self.xt.XTestFakeKeyEvent(self.display, code, down, 0)
        self.x.XFlush(self.display)
        time.sleep(.05)

    def tap(self, name, hold=()):
        for mod in hold:
            self.edge(mod, 1)
        self.edge(name, 1)
        self.edge(name, 0)
        for mod in reversed(hold):
            self.edge(mod, 0)
        time.sleep(.1)

    def text(self, value):
        """ASCII letters, digits and space (keysyms `space` for ' ')."""
        for c in value:
            self.tap('space' if c == ' ' else c)

    def pointer(self, x, y):
        """Window pixels (not GUI pixels): GUI scale times the GUI point."""
        self.xt.XTestFakeMotionEvent(self.display, -1, 0, 0, 0)
        self.x.XFlush(self.display)
        time.sleep(.05)
        self.xt.XTestFakeMotionEvent(self.display, -1, int(x), int(y), 0)
        self.x.XFlush(self.display)
        time.sleep(.2)
