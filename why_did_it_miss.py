import json

with open('mutants.out/outcomes.json', 'r') as f:
    outcomes = json.load(f)

for o in outcomes:
    if o.get('status', '').lower() == 'missed':
        print(o['mutant'])
