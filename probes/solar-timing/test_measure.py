import json
import unittest

from measure import decode_measurement


class MeasurementTests(unittest.TestCase):
    def test_valid_measurement(self) -> None:
        self.assertEqual(
            decode_measurement(
                '[{"first_micros": 15, "samples_micros": [10.5, 0], '
                '"rises": 1, "sets": 2}]',
                2,
            ),
            {
                "first_micros": 15.0,
                "samples_micros": [10.5, 0.0],
                "rises": 1,
                "sets": 2,
            },
        )

    def test_rejects_malformed_or_wrong_response_shape(self) -> None:
        for output in ("{", "{}", "[]", "[{}, {}]", "[null]", "[true]"):
            with self.subTest(output=output), self.assertRaises(ValueError):
                decode_measurement(output, 2)

    def test_rejects_missing_fields(self) -> None:
        record: dict[str, object] = {
            "first_micros": 15.0,
            "samples_micros": [10.0, 20.0],
            "rises": 1,
            "sets": 1,
        }
        for field in record:
            incomplete = record.copy()
            del incomplete[field]
            with self.subTest(field=field), self.assertRaises(ValueError):
                decode_measurement(json.dumps([incomplete]), 2)

    def test_rejects_invalid_measurement_values(self) -> None:
        cases: tuple[tuple[str, object], ...] = (
            ("first_micros", True),
            ("first_micros", -1.0),
            ("first_micros", "15"),
            ("first_micros", float("nan")),
            ("first_micros", float("inf")),
            ("samples_micros", "10,20"),
            ("samples_micros", []),
            ("samples_micros", [10.0]),
            ("samples_micros", [10.0, 20.0, 30.0]),
            ("samples_micros", [True, 20.0]),
            ("samples_micros", [10.0, -1.0]),
            ("samples_micros", [10.0, float("nan")]),
            ("samples_micros", [10.0, float("inf")]),
            ("rises", True),
            ("rises", -1),
            ("rises", 1.5),
            ("sets", "1"),
            ("sets", None),
        )
        for field, value in cases:
            record: dict[str, object] = {
                "first_micros": 15.0,
                "samples_micros": [10.0, 20.0],
                "rises": 1,
                "sets": 1,
            }
            record[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                decode_measurement(json.dumps([record]), 2)


if __name__ == "__main__":
    unittest.main()
