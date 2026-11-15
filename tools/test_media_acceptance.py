import unittest

from media_acceptance import assess_marks, audio_clock, av_alignment, barcode, frame, tone_power
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

    def test_av_transition_bound_rejects_delayed_audio(self):
        valid = [(index / 10, index) for index in range(100)]
        for delay in (0, 0.1, -0.1):
            windows = [(index / 10, max(0, math.floor(index / 10 + 0.05 - delay)), {})
                       for index in range(2, 98)]
            alignment = av_alignment(valid, windows)
            self.assertLessEqual(alignment["absolute_upper_seconds"], 0.5)
        for delay in (0.5, 0.8, -0.8, 2.0, 4.0):
            windows = [(index / 10, max(0, math.floor(index / 10 + 0.05 - delay)), {})
                       for index in range(100)]
            with self.assertRaisesRegex(AssertionError, "A/V"):
                av_alignment(valid, windows)

    def test_av_uses_shared_presentation_clock_and_unique_source_identity(self):
        valid = [(1.25 + index / 10, index) for index in range(100)]
        windows = [(1.25 + index / 10, index // 10, {}) for index in range(100)]
        self.assertLessEqual(av_alignment(valid, windows)["absolute_upper_seconds"], 0.5)
        with self.assertRaisesRegex(AssertionError, "A/V"):
            av_alignment(valid, [(at + 0.8, bit, powers) for at, bit, powers in windows])

    def test_audio_timestamp_gaps_and_accumulated_drift_cannot_be_concatenated(self):
        frames = [{"best_effort_timestamp_time": round(index * 1024 / 48000, 3), "nb_samples": 1024}
                  for index in range(100)]
        self.assertEqual(audio_clock(frames)["samples"], 102400)
        for step, drift in ((0.8, 0), (0, 0.001)):
            bad = [dict(frame, best_effort_timestamp_time=frame["best_effort_timestamp_time"]
                        + (step if index > 20 else 0) + index * drift)
                   for index, frame in enumerate(frames)]
            with self.assertRaisesRegex(AssertionError, "A/V audio PTS discontinuity"):
                audio_clock(bad)

    def test_av_uncertainty_includes_missing_video_samples(self):
        valid = [(index / 10, index) for index in range(100) if index not in (8, 9, 10, 11)]
        windows = [(index / 10, index // 10, {}) for index in range(100)]
        self.assertGreater(av_alignment(valid, windows)["absolute_upper_seconds"], 0.44)

    def test_av_boundaries_cannot_hide_wrong_source_content(self):
        valid = [(index / 10, index + 30) for index in range(100)]
        correct = [(index / 10, (index + 30) // 10, {}) for index in range(100)]
        av_alignment(valid, correct)
        for start, end in ((0, 4), (96, 100)):
            bad = [(at, 30 if start <= index < end else second, powers)
                   for index, (at, second, powers) in enumerate(correct)]
            with self.assertRaisesRegex(AssertionError, "A/V audio source identity"):
                av_alignment(valid, bad)


if __name__ == "__main__":
    unittest.main()
