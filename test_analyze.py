import json

def analyze():
    # What properties should we use for zombie_field_count?
    # The requirement says:
    # "identify potential "zombie" fields (e.g., custom fields that are rarely populated or unused in page layouts/queries)."
    # From SObjectDescribe, we can't tell if it's "rarely populated" (that requires FieldUsageScanner).
    # But we can tell if it's "unused in page layouts" implicitly if it's deprecated_and_hidden? No, deprecated_and_hidden is rarely used.
    # What about: custom && !required && not standard ?
    pass
