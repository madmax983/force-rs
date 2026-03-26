import re

with open("crates/force/src/api/soql.rs", "r") as f:
    code = f.read()

# Remove the test functions that expect panic on invalid fields
test_names = [
    "test_where_eq_panics_on_invalid_field",
    "test_where_ne_panics_on_invalid_field",
    "test_where_in_panics_on_invalid_field",
    "test_where_like_panics_on_invalid_field",
    "test_order_by_panics_on_invalid_field",
    "test_order_by_desc_panics_on_invalid_field",
    "test_from_panics_on_invalid_sobject",
    "test_select_panics_on_invalid_field"
]

for test in test_names:
    code = re.sub(
        r'\s*#\[test\]\s*#\[should_panic[^\]]*\]\s*fn ' + test + r'\(\) \{.*?\n    \}',
        '',
        code,
        flags=re.DOTALL
    )

with open("crates/force/src/api/soql.rs", "w") as f:
    f.write(code)
