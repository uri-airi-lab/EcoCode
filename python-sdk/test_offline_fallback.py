import os
import unittest
import warnings
import ecocode

class TestOfflineFallback(unittest.TestCase):
    def test_offline_backend_fallback(self):
        # Set environment variables to simulate an offline server on a random port
        os.environ["ECOCODE_API_URL"] = "http://localhost:59999"
        os.environ["ECOCODE_API_KEY"] = "eco_12345678901234567890123456789012"

        # We expect a warning to be raised since the backend is offline
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            
            # This should NOT raise an exception and should successfully fall back
            with ecocode.run("test-offline-run") as run:
                # Verify it set the mode to noop
                self.assertEqual(run._mode, "noop")
            
            # Verify the warning was issued
            self.assertTrue(any("EcoCode backend unreachable" in str(warn.message) for warn in w))

if __name__ == "__main__":
    unittest.main()
