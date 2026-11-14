import unittest

from media_acceptance import assess_marks, barcode, frame, tone_power
import math


class MediaOracleTests(unittest.TestCase):
    def test_generated_identity_survives_independent_pixel_reader(self):
        for tick in (0, 1, 20, 255, 449):
            for epoch in (1, 7, 15):
                self.assertEqual(barcode(frame(tick, epoch, tick * 3)), (tick, epoch))

    def test_black_and_corrupt_identity_are_rejected(self):
        self.assertIsNone(barcode(bytes(1280 * 720)))
        pixels = frame(10, 2, 30)
        pixels[650 * 1280 + 42 + 15 * 70] = 235 - pixels[650 * 1280 + 42 + 15 * 70]
        self.assertIsNone(barcode(pixels))

    def test_content_gate_rejects_frozen_backward_and_old_session_frames(self):
        valid, span = assess_marks([(tick, 3) for tick in range(100)], 3)
        self.assertEqual(len(valid), 100)
        self.assertEqual(span, 9.9)
        for marks, reason in (
            ([(20, 3)] * 100, "frozen"),
            ([(tick, 3) for tick in reversed(range(100))], "backward"),
            ([(tick, 2) for tick in range(100)], "wrong source"),
        ):
            with self.assertRaisesRegex(AssertionError, reason):
                assess_marks(marks, 3)

    def test_partial_freezes_cannot_hide_behind_total_progression(self):
        for marks in (
            [(0, 3)] * 20 + [(tick, 3) for tick in range(80)],
            [(tick, 3) for tick in range(40)] + [(39, 3)] * 20 + [(tick, 3) for tick in range(40, 80)],
            [(tick, 3) for tick in range(71)] + [(70, 3)] * 29,
        ):
            with self.assertRaisesRegex(AssertionError, "frozen"):
                assess_marks(marks, 3)

    def test_audio_classifier_identifies_frequency_in_pcm(self):
        samples = [int(5000 * math.sin(2 * math.pi * 440 * n / 48000)) for n in range(4800)]
        self.assertGreater(tone_power(samples, 440), 100 * tone_power(samples, 880))


if __name__ == "__main__":
    unittest.main()
