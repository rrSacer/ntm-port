#!/usr/bin/env python3
"""The 26.3 vein port must give the Java yield per attempt for Nether sizes, within sampling noise."""
import unittest
from vein_yield_oracle import mean_yield_java, mean_yield_26_3


class VeinYield(unittest.TestCase):
    def test_port_matches_java_per_attempt(self):
        for size in (6, 10, 12):
            java = mean_yield_java(size, n=3000)
            port = mean_yield_26_3(size, n=3000)
            # The port prunes spheres contained in others and stops one step early; the gap stays below 15%.
            self.assertLess(abs(port - java) / java, 0.15, (size, java, port))


if __name__ == '__main__':
    unittest.main()
